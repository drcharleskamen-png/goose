use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::config::Config;

/// Lifecycle hook events that can be intercepted
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    BeforeToolCall,
    AfterToolCall,
    OnSessionStart,
    OnSessionEnd,
    BeforeReply,
    AfterReply,
}

impl std::fmt::Display for HookEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookEvent::BeforeToolCall => write!(f, "before_tool_call"),
            HookEvent::AfterToolCall => write!(f, "after_tool_call"),
            HookEvent::OnSessionStart => write!(f, "on_session_start"),
            HookEvent::OnSessionEnd => write!(f, "on_session_end"),
            HookEvent::BeforeReply => write!(f, "before_reply"),
            HookEvent::AfterReply => write!(f, "after_reply"),
        }
    }
}

/// A single hook handler (command or HTTP)
#[derive(Debug, Clone, Deserialize)]
pub struct HookHandler {
    /// Shell command to execute (receives JSON on stdin)
    pub command: Option<String>,
    /// HTTP URL to POST to (receives JSON body)
    pub url: Option<String>,
    /// Timeout in seconds (default: 10)
    pub timeout: Option<u64>,
}

/// A hook entry that pairs a matcher with one or more handlers
#[derive(Debug, Clone, Deserialize)]
pub struct HookEntry {
    /// Regex pattern to match tool names (only for tool events, optional)
    pub matcher: Option<String>,
    /// Handlers to execute when this hook fires
    pub hooks: Vec<HookHandler>,
}

/// Context passed to hooks as JSON on stdin / HTTP body
#[derive(Debug, Clone, Serialize)]
pub struct HookContext {
    pub event: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
}

/// Decision returned by a hook
#[derive(Debug, Clone, Default)]
pub struct HookDecision {
    /// Block the action (for before_tool_call: deny the tool call)
    pub block: bool,
    /// Reason for blocking
    pub reason: Option<String>,
}

/// The hooks manager - loads config and dispatches events
pub struct HookManager {
    hooks: HashMap<HookEvent, Vec<HookEntry>>,
}

impl HookManager {
    pub fn new() -> Self {
        Self {
            hooks: HashMap::new(),
        }
    }

    /// Load hooks from goose config.yaml
    pub fn from_config() -> Self {
        let mut manager = Self::new();
        if let Err(e) = manager.load_from_config() {
            tracing::warn!(error = %e, "Failed to load hooks from config");
        }
        manager
    }

    fn load_from_config(&mut self) -> Result<()> {
        let config = Config::global();
        let hooks_value: Value = match config.get_param("hooks") {
            Ok(v) => v,
            Err(_) => return Ok(()), // No hooks configured
        };

        let hooks_map: HashMap<HookEvent, Vec<HookEntry>> = serde_json::from_value(hooks_value)?;
        self.hooks = hooks_map;

        let total: usize = self.hooks.values().map(|v| v.len()).sum();
        if total > 0 {
            tracing::info!(
                hook_count = total,
                events = ?self.hooks.keys().collect::<Vec<_>>(),
                "Loaded lifecycle hooks"
            );
        }

        Ok(())
    }

    /// Check if any hooks are registered for an event
    pub fn has_hooks(&self, event: HookEvent) -> bool {
        self.hooks
            .get(&event)
            .is_some_and(|entries| !entries.is_empty())
    }

    /// Emit a hook event and collect decisions
    pub async fn emit(&self, event: HookEvent, ctx: HookContext) -> HookDecision {
        let entries = match self.hooks.get(&event) {
            Some(entries) => entries,
            None => return HookDecision::default(),
        };

        let ctx_json = match serde_json::to_string(&ctx) {
            Ok(j) => j,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to serialize hook context");
                return HookDecision::default();
            }
        };

        let mut decision = HookDecision::default();

        for entry in entries {
            // Check matcher against tool name
            if let Some(ref matcher) = entry.matcher {
                if let Some(ref tool_name) = ctx.tool_name {
                    match Regex::new(matcher) {
                        Ok(re) => {
                            if !re.is_match(tool_name) {
                                continue;
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                matcher = matcher,
                                error = %e,
                                "Invalid hook matcher regex, skipping"
                            );
                            continue;
                        }
                    }
                }
            }

            for handler in &entry.hooks {
                let timeout_secs = handler.timeout.unwrap_or(10);

                if let Some(ref cmd) = handler.command {
                    match execute_command_hook(cmd, &ctx_json, timeout_secs).await {
                        Ok(Some(d)) => {
                            if d.block {
                                decision = d;
                                return decision; // Short-circuit on first block
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            tracing::warn!(
                                command = cmd,
                                event = %event,
                                error = %e,
                                "Hook command failed"
                            );
                        }
                    }
                }

                if let Some(ref url) = handler.url {
                    match execute_http_hook(url, &ctx_json, timeout_secs).await {
                        Ok(Some(d)) => {
                            if d.block {
                                decision = d;
                                return decision;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            tracing::warn!(
                                url = url,
                                event = %event,
                                error = %e,
                                "Hook HTTP request failed"
                            );
                        }
                    }
                }
            }
        }

        decision
    }
}

impl Default for HookManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Execute a shell command hook. Returns a decision if the command produces JSON output.
/// Exit code 2 = block (Claude Code convention).
async fn execute_command_hook(
    cmd: &str,
    ctx_json: &str,
    timeout_secs: u64,
) -> Result<Option<HookDecision>> {
    let expanded = shellexpand::tilde(cmd);

    let parts: Vec<&str> = expanded.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(None);
    }

    let mut child = Command::new(parts[0])
        .args(&parts[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(ctx_json.as_bytes()).await;
        let _ = stdin.shutdown().await;
    }

    let output = tokio::time::timeout(Duration::from_secs(timeout_secs), child.wait_with_output())
        .await
        .map_err(|_| anyhow::anyhow!("Hook command timed out after {}s", timeout_secs))??;

    // Exit code 2 = block (Claude Code convention)
    if output.status.code() == Some(2) {
        let reason = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Ok(Some(HookDecision {
            block: true,
            reason: if reason.is_empty() {
                None
            } else {
                Some(reason)
            },
        }));
    }

    // Try to parse stdout as JSON decision
    if !output.stdout.is_empty() {
        if let Ok(json) = serde_json::from_slice::<Value>(&output.stdout) {
            return Ok(Some(parse_decision(&json)));
        }
    }

    Ok(None)
}

/// Execute an HTTP POST hook
async fn execute_http_hook(
    url: &str,
    ctx_json: &str,
    timeout_secs: u64,
) -> Result<Option<HookDecision>> {
    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .header("Content-Type", "application/json")
        .body(ctx_json.to_string())
        .timeout(Duration::from_secs(timeout_secs))
        .send()
        .await?;

    if resp.status().is_success() {
        let body = resp.text().await?;
        if !body.is_empty() {
            if let Ok(json) = serde_json::from_str::<Value>(&body) {
                return Ok(Some(parse_decision(&json)));
            }
        }
    }

    Ok(None)
}

/// Parse a JSON response into a HookDecision.
/// Supports both Claude Code style (decision/reason) and Hermes style (action/message).
fn parse_decision(json: &Value) -> HookDecision {
    // Claude Code style: {"decision": "block", "reason": "..."}
    if let Some(decision) = json.get("decision").and_then(|v| v.as_str()) {
        if decision == "block" || decision == "deny" {
            return HookDecision {
                block: true,
                reason: json
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            };
        }
    }

    // Hermes style: {"action": "block", "message": "..."}
    if let Some(action) = json.get("action").and_then(|v| v.as_str()) {
        if action == "block" || action == "deny" {
            return HookDecision {
                block: true,
                reason: json
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            };
        }
    }

    // Direct: {"block": true, "reason": "..."}
    if json.get("block").and_then(|v| v.as_bool()).unwrap_or(false) {
        return HookDecision {
            block: true,
            reason: json
                .get("reason")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        };
    }

    HookDecision::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_decision_claude_style() {
        let json: Value =
            serde_json::from_str(r#"{"decision": "block", "reason": "dangerous"}"#).unwrap();
        let d = parse_decision(&json);
        assert!(d.block);
        assert_eq!(d.reason.as_deref(), Some("dangerous"));
    }

    #[test]
    fn test_parse_decision_hermes_style() {
        let json: Value =
            serde_json::from_str(r#"{"action": "deny", "message": "not allowed"}"#).unwrap();
        let d = parse_decision(&json);
        assert!(d.block);
        assert_eq!(d.reason.as_deref(), Some("not allowed"));
    }

    #[test]
    fn test_parse_decision_allow() {
        let json: Value = serde_json::from_str(r#"{"decision": "allow"}"#).unwrap();
        let d = parse_decision(&json);
        assert!(!d.block);
    }

    #[test]
    fn test_parse_decision_direct() {
        let json: Value = serde_json::from_str(r#"{"block": true, "reason": "nope"}"#).unwrap();
        let d = parse_decision(&json);
        assert!(d.block);
        assert_eq!(d.reason.as_deref(), Some("nope"));
    }

    #[test]
    fn test_hook_event_display() {
        assert_eq!(HookEvent::BeforeToolCall.to_string(), "before_tool_call");
        assert_eq!(HookEvent::AfterToolCall.to_string(), "after_tool_call");
        assert_eq!(HookEvent::OnSessionStart.to_string(), "on_session_start");
    }

    #[test]
    fn test_hook_manager_empty() {
        let manager = HookManager::new();
        assert!(!manager.has_hooks(HookEvent::BeforeToolCall));
    }

    #[tokio::test]
    async fn test_emit_no_hooks() {
        let manager = HookManager::new();
        let ctx = HookContext {
            event: "before_tool_call".to_string(),
            session_id: "test".to_string(),
            tool_name: Some("bash".to_string()),
            tool_input: None,
            tool_result: None,
            message: None,
            working_dir: None,
        };
        let decision = manager.emit(HookEvent::BeforeToolCall, ctx).await;
        assert!(!decision.block);
    }

    #[test]
    fn test_deserialize_hook_config() {
        let yaml = r#"
before_tool_call:
  - matcher: "bash|write_file"
    hooks:
      - command: "~/.config/goose/hooks/check-safety.sh"
        timeout: 5
after_tool_call:
  - hooks:
      - url: "http://localhost:9000/hook"
"#;
        let hooks: HashMap<HookEvent, Vec<HookEntry>> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(hooks.len(), 2);
        assert_eq!(hooks[&HookEvent::BeforeToolCall].len(), 1);
        assert_eq!(
            hooks[&HookEvent::BeforeToolCall][0].matcher.as_deref(),
            Some("bash|write_file")
        );
        assert_eq!(hooks[&HookEvent::BeforeToolCall][0].hooks.len(), 1);
        assert_eq!(
            hooks[&HookEvent::BeforeToolCall][0].hooks[0]
                .command
                .as_deref(),
            Some("~/.config/goose/hooks/check-safety.sh")
        );
    }
}
