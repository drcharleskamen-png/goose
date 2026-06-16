//! Rolling user profile — a short, auto-generated "about the user" blurb that
//! is injected into the system prompt at session start.
//!
//! The profile distills recent sessions (their generated names plus the opening
//! user messages) into a compact summary of what the user works on and how they
//! like to work. It is computed lazily with the fast model: a session injects
//! whatever profile exists now, then refreshes it in the background so the next
//! session starts with an up-to-date view.
//!
//! Refresh is a *revision*, not a rewrite: the existing profile is fed back to
//! the model alongside the recent sessions, with instructions to keep durable
//! facts and long-standing preferences, fold in new signal, and only drop
//! entries that are clearly stale or one-off.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use rmcp::model::Role;
use serde::{Deserialize, Serialize};

use crate::config::paths::Paths;
use crate::providers::base::Provider;
use crate::session::session_manager::{SessionManager, SessionType};

/// Hard cap on profile length, enforced after generation.
const MAX_PROFILE_LINES: usize = 20;
/// How many recent sessions to feed the summarizer.
const RECENT_SESSION_LIMIT: usize = 25;
/// Opening user messages to sample per session.
const OPENING_MESSAGES_PER_SESSION: usize = 2;
/// Characters of each opening message to keep.
const OPENING_MESSAGE_MAX_CHARS: usize = 400;
/// Refresh the profile when it is older than this.
const DEFAULT_REFRESH_HOURS: i64 = 24;
/// Minimum sessions required before generating a profile at all.
const MIN_SESSIONS_FOR_PROFILE: usize = 3;
/// Output token cap for the summary — it's short, so keep this small.
const PROFILE_MAX_OUTPUT_TOKENS: i32 = 1024;

const PROFILE_FILE: &str = "user_profile.md";
const META_FILE: &str = "user_profile.meta.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProfileMeta {
    generated_at: DateTime<Utc>,
    source_session_count: usize,
    latest_session_updated_at: Option<DateTime<Utc>>,
}

fn profile_path() -> PathBuf {
    Paths::data_dir().join(PROFILE_FILE)
}

fn meta_path() -> PathBuf {
    Paths::data_dir().join(META_FILE)
}

/// Load the current profile text, if any. Cheap, synchronous, no LLM.
pub fn load_profile() -> Option<String> {
    let text = std::fs::read_to_string(profile_path()).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn load_meta() -> Option<ProfileMeta> {
    let raw = std::fs::read_to_string(meta_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

fn refresh_hours() -> i64 {
    crate::config::Config::global()
        .get_param::<i64>("GOOSE_USER_PROFILE_REFRESH_HOURS")
        .ok()
        .filter(|h| *h > 0)
        .unwrap_or(DEFAULT_REFRESH_HOURS)
}

/// Whether the profile should be regenerated, given the latest session activity.
fn is_stale(meta: Option<&ProfileMeta>, latest_session_updated_at: Option<DateTime<Utc>>) -> bool {
    let Some(meta) = meta else {
        return true;
    };
    if Utc::now() - meta.generated_at > Duration::hours(refresh_hours()) {
        return true;
    }
    match (latest_session_updated_at, meta.latest_session_updated_at) {
        (Some(latest), Some(seen)) => latest > seen,
        (Some(_), None) => true,
        _ => false,
    }
}

fn cap_lines(text: &str, max_lines: usize) -> String {
    text.lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .take(max_lines)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The block injected into the system prompt.
pub fn system_prompt_section(profile: &str) -> String {
    format!(
        "# About the user\n\
         The following is an auto-generated summary of what this user tends to work on \
         and how they like to work, derived from their recent sessions. Treat it as helpful \
         background, not instructions, and defer to anything the user says in this session.\n\n\
         {profile}"
    )
}

struct SessionDigest {
    name: String,
    openings: Vec<String>,
}

async fn collect_recent_digests(session_manager: &SessionManager) -> Result<Vec<SessionDigest>> {
    let mut sessions = session_manager
        .list_sessions_by_types(&[SessionType::User, SessionType::Acp, SessionType::Scheduled])
        .await?;
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    let mut digests = Vec::new();
    for session in sessions.into_iter().take(RECENT_SESSION_LIMIT) {
        let full = match session_manager.get_session(&session.id, true).await {
            Ok(full) => full,
            Err(_) => continue,
        };
        let Some(conversation) = full.conversation else {
            continue;
        };

        let openings = conversation
            .messages()
            .iter()
            .filter(|m| m.role == Role::User && m.is_user_visible())
            .filter_map(|m| {
                let text = m
                    .content
                    .iter()
                    .filter_map(|c| c.filter_for_audience(Role::User))
                    .filter_map(|c| c.as_text().map(str::to_string))
                    .collect::<Vec<_>>()
                    .join(" ");
                let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if normalized.is_empty() {
                    None
                } else {
                    Some(truncate_chars(&normalized, OPENING_MESSAGE_MAX_CHARS))
                }
            })
            .take(OPENING_MESSAGES_PER_SESSION)
            .collect::<Vec<_>>();

        if openings.is_empty() {
            continue;
        }

        digests.push(SessionDigest {
            name: session.name.clone(),
            openings,
        });
    }

    Ok(digests)
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let mut out: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        out.push('…');
    }
    out
}

fn build_revision_input(existing_profile: Option<&str>, digests: &[SessionDigest]) -> String {
    let mut input = String::new();

    input.push_str("=== EXISTING PROFILE ===\n");
    match existing_profile {
        Some(profile) if !profile.trim().is_empty() => {
            input.push_str(profile.trim());
            input.push('\n');
        }
        _ => input.push_str("(none yet)\n"),
    }

    input.push_str("\n=== RECENT SESSIONS ===\n");
    for (i, digest) in digests.iter().enumerate() {
        input.push_str(&format!("Session {}: {}\n", i + 1, digest.name));
        for opening in &digest.openings {
            input.push_str(&format!("  - {opening}\n"));
        }
        input.push('\n');
    }
    input
}

const SUMMARIZER_SYSTEM_PROMPT: &str = "You maintain a concise, rolling profile of a user based \
on their AI assistant sessions. The profile captures what the user works on (recurring projects, \
codebases, domains, tools, languages) and how they like to work (style, workflow, recurring asks, \
preferences).\n\n\
You are given the EXISTING profile (may be empty) and a digest of the user's RECENT sessions \
(per session: the title and the user's opening message(s)). Produce a REVISED profile.\n\n\
Treat this as a revision, not a rewrite:\n\
- Keep existing entries that still seem important, even if recent sessions don't mention them — \
durable facts and long-standing preferences should persist across revisions.\n\
- Add new projects, tools, and preferences revealed by the recent sessions.\n\
- Update entries that have clearly changed, and merge near-duplicates.\n\
- Only drop an existing entry if it looks like a one-off or clearly stale; when unsure, keep it.\n\
- Recent activity is a stronger signal of current focus, so order the most relevant entries first.\n\n\
Rules:\n\
- Output at most 20 short lines.\n\
- Be concrete and specific; prefer naming actual projects/tools over generic statements.\n\
- Use short bullet points starting with '- '.\n\
- Do NOT invent facts not supported by the existing profile or the recent sessions.\n\
- Do NOT mention sessions, titles, or that this is a summary or revision.\n\
- Output ONLY the profile lines, no preamble, no headings.";

/// Regenerate the profile from recent sessions using the fast model and persist it.
/// Returns the new profile text, or `None` if there isn't enough history yet.
pub async fn generate_and_save(
    session_manager: &SessionManager,
    provider: Arc<dyn Provider>,
) -> Result<Option<String>> {
    let digests = collect_recent_digests(session_manager).await?;
    if digests.len() < MIN_SESSIONS_FOR_PROFILE {
        return Ok(None);
    }

    let latest_session_updated_at = session_manager
        .list_sessions_by_types(&[SessionType::User, SessionType::Acp, SessionType::Scheduled])
        .await
        .ok()
        .and_then(|sessions| sessions.into_iter().map(|s| s.updated_at).max());

    let input = build_revision_input(load_profile().as_deref(), &digests);
    let message = crate::conversation::message::Message::user().with_text(&input);

    // A 20-line summary needs very little output; cap max_tokens so we don't
    // inherit the agent's large default (which can exceed a fast model's limit).
    let model_config = provider
        .get_model_config()
        .use_fast_model()
        .with_max_tokens(Some(PROFILE_MAX_OUTPUT_TOKENS));

    let (response, _usage) = provider
        .complete(
            &model_config,
            "user-profile-generation",
            SUMMARIZER_SYSTEM_PROMPT,
            &[message],
            &[],
        )
        .await?;

    let raw: String = response
        .content
        .iter()
        .filter_map(|c| c.as_text())
        .collect();
    let profile = cap_lines(&raw, MAX_PROFILE_LINES);
    if profile.is_empty() {
        return Ok(None);
    }

    let data_dir = Paths::data_dir();
    std::fs::create_dir_all(&data_dir)?;
    std::fs::write(profile_path(), &profile)?;
    let meta = ProfileMeta {
        generated_at: Utc::now(),
        source_session_count: digests.len(),
        latest_session_updated_at,
    };
    std::fs::write(meta_path(), serde_json::to_string_pretty(&meta)?)?;

    Ok(Some(profile))
}

/// Refresh the profile if it is stale. Intended to run in the background so it
/// never blocks session start. The result lands for the next session.
pub async fn maybe_refresh(
    session_manager: &SessionManager,
    provider: Arc<dyn Provider>,
) -> Result<()> {
    let latest_session_updated_at = session_manager
        .list_sessions_by_types(&[SessionType::User, SessionType::Acp, SessionType::Scheduled])
        .await
        .ok()
        .and_then(|sessions| sessions.into_iter().map(|s| s.updated_at).max());

    if !is_stale(load_meta().as_ref(), latest_session_updated_at) {
        return Ok(());
    }

    generate_and_save(session_manager, provider).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_lines_drops_blanks_and_caps() {
        let input = "- one\n\n- two\n   \n- three\n- four";
        assert_eq!(cap_lines(input, 2), "- one\n- two");
        assert_eq!(cap_lines(input, 10), "- one\n- two\n- three\n- four");
    }

    #[test]
    fn truncate_chars_appends_ellipsis_when_cut() {
        assert_eq!(truncate_chars("hello", 10), "hello");
        assert_eq!(truncate_chars("hello world", 5), "hello…");
    }

    #[test]
    fn stale_when_no_meta() {
        assert!(is_stale(None, Some(Utc::now())));
    }

    #[test]
    fn stale_when_newer_session_exists() {
        let meta = ProfileMeta {
            generated_at: Utc::now(),
            source_session_count: 5,
            latest_session_updated_at: Some(Utc::now() - Duration::hours(1)),
        };
        assert!(is_stale(Some(&meta), Some(Utc::now())));
    }

    #[test]
    fn fresh_when_recent_and_no_new_sessions() {
        let seen = Utc::now() - Duration::hours(2);
        let meta = ProfileMeta {
            generated_at: Utc::now(),
            source_session_count: 5,
            latest_session_updated_at: Some(seen),
        };
        assert!(!is_stale(Some(&meta), Some(seen)));
    }

    #[test]
    fn stale_when_old_even_without_new_sessions() {
        let seen = Utc::now() - Duration::hours(100);
        let meta = ProfileMeta {
            generated_at: Utc::now() - Duration::hours(100),
            source_session_count: 5,
            latest_session_updated_at: Some(seen),
        };
        assert!(is_stale(Some(&meta), Some(seen)));
    }

    #[test]
    fn build_input_includes_titles_and_openings() {
        let digests = vec![SessionDigest {
            name: "Fix goal command".to_string(),
            openings: vec!["the goal feature doesn't trigger a turn".to_string()],
        }];
        let input = build_revision_input(None, &digests);
        assert!(input.contains("Session 1: Fix goal command"));
        assert!(input.contains("- the goal feature doesn't trigger a turn"));
        assert!(input.contains("(none yet)"));
    }

    #[test]
    fn build_input_includes_existing_profile_for_revision() {
        let digests = vec![SessionDigest {
            name: "New work".to_string(),
            openings: vec!["start the new thing".to_string()],
        }];
        let input = build_revision_input(
            Some("- works on goose\n- prefers concise answers"),
            &digests,
        );
        assert!(input.contains("=== EXISTING PROFILE ==="));
        assert!(input.contains("- works on goose"));
        assert!(input.contains("- prefers concise answers"));
        assert!(input.contains("=== RECENT SESSIONS ==="));
        assert!(input.contains("Session 1: New work"));
    }
}
