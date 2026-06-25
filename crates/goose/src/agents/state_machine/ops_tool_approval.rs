use anyhow::{anyhow, Result};
use async_trait::async_trait;
use rmcp::model::Role;

use crate::agents::state_machine::operation::{Emitter, Operation, OperationResult, TurnEffect};
use crate::agents::{Agent, AgentEvent};
use crate::config::permission::PermissionLevel;
use crate::conversation::message::{Message, MessageContent, ToolRequest};
use crate::permission::Permission;
use crate::session::Session;
use crate::tool_inspection::{get_security_finding_id_from_results, InspectionAction};

pub const TOOL_APPROVAL_KEY: &str = "goose.approval";
pub const TOOL_APPROVAL_ALLOWED: &str = "allowed";
pub const TOOL_APPROVAL_DENIED: &str = "denied";

pub struct ToolApprovalOperation<'a> {
    agent: &'a Agent,
}

impl<'a> ToolApprovalOperation<'a> {
    pub fn new(agent: &'a Agent) -> Self {
        Self { agent }
    }
}

#[async_trait]
impl Operation for ToolApprovalOperation<'_> {
    fn name(&self) -> &'static str {
        "tool_approval"
    }

    async fn run(&self, session: &Session, emit: Emitter) -> Result<OperationResult> {
        let Some(message) = session.conversation.as_ref().and_then(|c| c.last()) else {
            return Ok(OperationResult::NotApplicable(emit));
        };
        if message.role != Role::Assistant {
            return Ok(OperationResult::NotApplicable(emit));
        }

        let pending: Vec<ToolRequest> = message
            .content
            .iter()
            .filter_map(|content| match content {
                MessageContent::ToolRequest(request)
                    if request.tool_call.is_ok() && approval_state(request).is_none() =>
                {
                    Some(request.clone())
                }
                _ => None,
            })
            .collect();
        if pending.is_empty() {
            return Ok(OperationResult::NotApplicable(emit));
        }

        let message_id = message
            .id
            .clone()
            .ok_or_else(|| anyhow!("Persisted tool request message has no id"))?;
        let conversation = session.conversation.as_ref().expect("checked above");
        let goose_mode = self.agent.goose_mode().await;
        let inspection_results = self
            .agent
            .tool_inspection_manager
            .inspect_tools(&session.id, &pending, conversation.messages(), goose_mode)
            .await?;
        let permission_check_result = self
            .agent
            .tool_inspection_manager
            .process_inspection_results_with_permission_inspector(&pending, &inspection_results)
            .unwrap_or_else(
                || crate::permission::permission_judge::PermissionCheckResult {
                    approved: Vec::new(),
                    needs_approval: pending.clone(),
                    denied: Vec::new(),
                },
            );

        let mut effects = Vec::new();
        for request in permission_check_result.approved {
            effects.push(mark_decision(
                &message_id,
                &request.id,
                TOOL_APPROVAL_ALLOWED,
            ));
        }
        for request in permission_check_result.denied {
            effects.push(mark_decision(
                &message_id,
                &request.id,
                TOOL_APPROVAL_DENIED,
            ));
        }

        for request in permission_check_result.needs_approval {
            let tool_call = request.tool_call.clone()?;
            let security_message = inspection_results
                .iter()
                .find(|result| result.tool_request_id == request.id)
                .and_then(|result| match &result.action {
                    InspectionAction::RequireApproval(Some(message)) => Some(message.clone()),
                    _ => None,
                });

            let confirmation_rx = self
                .agent
                .tool_confirmation_router
                .register(request.id.clone())
                .await;

            let action_required = Message::assistant()
                .with_action_required(
                    request.id.clone(),
                    tool_call.name.to_string(),
                    tool_call.arguments.clone().unwrap_or_default(),
                    security_message,
                )
                .user_only();
            emit.emit(AgentEvent::Message(action_required)).await;

            let confirmation = confirmation_rx
                .await
                .map_err(|_| anyhow!("Confirmation channel closed for request {}", request.id))?;

            if let Some(finding_id) =
                get_security_finding_id_from_results(&request.id, &inspection_results)
            {
                let action = match confirmation.permission {
                    Permission::AllowOnce | Permission::AlwaysAllow => "ALLOW",
                    _ => "BLOCK",
                };
                tracing::info!(
                    monotonic_counter.goose.prompt_injection_user_decisions = 1,
                    security.event_type = "user_decision",
                    security.action = action,
                    security.finding_id = %finding_id,
                    tool.request_id = %request.id,
                    user.decision = ?confirmation.permission,
                    "security finding: user decision"
                );
            }

            if confirmation.permission == Permission::AllowOnce
                || confirmation.permission == Permission::AlwaysAllow
            {
                effects.push(mark_decision(
                    &message_id,
                    &request.id,
                    TOOL_APPROVAL_ALLOWED,
                ));
                if confirmation.permission == Permission::AlwaysAllow {
                    self.agent
                        .tool_inspection_manager
                        .update_permission_manager(&tool_call.name, PermissionLevel::AlwaysAllow)
                        .await;
                }
            } else {
                effects.push(mark_decision(
                    &message_id,
                    &request.id,
                    TOOL_APPROVAL_DENIED,
                ));
                if confirmation.permission == Permission::AlwaysDeny {
                    self.agent
                        .tool_inspection_manager
                        .update_permission_manager(&tool_call.name, PermissionLevel::NeverAllow)
                        .await;
                }
            }
        }

        Ok(OperationResult::Applied(effects))
    }
}

fn approval_state(request: &ToolRequest) -> Option<&str> {
    request
        .tool_meta
        .as_ref()
        .and_then(|meta| meta.get(TOOL_APPROVAL_KEY))
        .and_then(|value| value.as_str())
}

fn mark_decision(message_id: &str, tool_call_id: &str, decision: &str) -> TurnEffect {
    TurnEffect::PatchToolRequestMeta {
        message_id: message_id.to_string(),
        tool_call_id: tool_call_id.to_string(),
        patch: serde_json::json!({ TOOL_APPROVAL_KEY: decision }),
    }
}
