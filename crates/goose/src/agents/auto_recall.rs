//! Automatic chat recall — a "Remembering" step that runs before a turn.
//!
//! When a user message looks like it references past work ("remind me…",
//! "what did we decide about…", or just names a topic discussed before), this
//! retrieves the most relevant past sessions, injects them as grounded context
//! for the model, and surfaces the sources (title + date) to the user — like
//! ChatGPT's recall panel.
//!
//! It is opt-in (`GOOSE_CHAT_RECALL_AUTO`) and cheap: a fast-model gate decides
//! whether recall is warranted and extracts search keywords, returning nothing
//! for trivial messages so most turns pay no recall cost.

use std::sync::Arc;

use crate::providers::base::Provider;
use crate::session::chat_history_search::{ChatRecallMessage, ChatRecallResult};
use crate::session::session_manager::{SessionManager, SessionType};

const MAX_RECALLED_SESSIONS: usize = 3;
const MAX_SNIPPET_CHARS: usize = 300;
const KEYWORD_GATE_MAX_TOKENS: i32 = 64;

/// A single recalled source, ready to cite.
pub struct RecalledSource {
    pub session_id: String,
    pub title: String,
    pub date: String,
    pub snippet: String,
}

pub struct RecallOutcome {
    pub sources: Vec<RecalledSource>,
    /// Agent-visible grounded context to inject before the turn.
    pub context_block: String,
    /// User-visible one-liner summarizing what was recalled.
    pub user_summary: String,
}

const GATE_SYSTEM_PROMPT: &str = "You decide whether a user's message to an AI assistant would \
benefit from recalling PAST conversations, and if so, what to search for.\n\n\
Recall is warranted when the message references prior work, decisions, or facts the user expects \
you to remember (e.g. 'remind me…', 'what did we decide about…', 'the X we set up', or naming a \
specific project/topic likely discussed before). It is NOT warranted for greetings, brand-new \
requests, or self-contained questions.\n\n\
Reply with ONLY a line of space-separated search keywords (include useful synonyms) if recall is \
warranted, or exactly the word NONE if it is not. No other text.";

/// Ask the fast model whether to recall and for what keywords.
/// Returns `None` to skip recall. Falls back to a simple heuristic on error.
async fn extract_keywords(provider: &Arc<dyn Provider>, user_text: &str) -> Option<String> {
    if user_text.trim().is_empty() {
        return None;
    }

    let model_config = provider
        .get_model_config()
        .use_fast_model()
        .with_max_tokens(Some(KEYWORD_GATE_MAX_TOKENS));

    let message = crate::conversation::message::Message::user().with_text(user_text);
    let result = provider
        .complete(
            &model_config,
            "auto-recall-gate",
            GATE_SYSTEM_PROMPT,
            &[message],
            &[],
        )
        .await;

    match result {
        Ok((response, _usage)) => {
            let raw: String = response
                .content
                .iter()
                .filter_map(|c| c.as_text())
                .collect();
            let cleaned = raw.trim();
            if cleaned.is_empty() || cleaned.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(cleaned.split_whitespace().collect::<Vec<_>>().join(" "))
            }
        }
        Err(_) => heuristic_keywords(user_text),
    }
}

/// Cheap fallback: only recall when the message explicitly invokes memory.
fn heuristic_keywords(user_text: &str) -> Option<String> {
    const TRIGGERS: &[&str] = &[
        "remind me",
        "remember",
        "we discussed",
        "we decided",
        "last time",
        "earlier you",
        "what did we",
        "recall",
    ];
    let lower = user_text.to_lowercase();
    if !TRIGGERS.iter().any(|t| lower.contains(t)) {
        return None;
    }
    let keywords: Vec<String> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 3)
        .take(8)
        .map(str::to_string)
        .collect();
    if keywords.is_empty() {
        None
    } else {
        Some(keywords.join(" "))
    }
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = normalized.chars();
    let mut out: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        out.push('…');
    }
    out
}

fn best_snippet(result: &ChatRecallResult) -> String {
    let pick = result
        .messages
        .iter()
        .find(|m: &&ChatRecallMessage| m.role.eq_ignore_ascii_case("user"))
        .or_else(|| result.messages.first());
    pick.map(|m| truncate_chars(&m.content, MAX_SNIPPET_CHARS))
        .unwrap_or_default()
}

/// A citable title: the session's generated name, or a short snippet-derived
/// fallback when the session was never named.
fn session_title(description: &str, snippet: &str) -> String {
    let trimmed = description.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    let fallback = truncate_chars(snippet, 48);
    if fallback.is_empty() {
        "Untitled session".to_string()
    } else {
        fallback
    }
}

fn search_session_types(current: Option<SessionType>) -> Vec<SessionType> {
    match current {
        Some(SessionType::Acp) => vec![SessionType::Acp],
        _ => vec![SessionType::User, SessionType::Scheduled],
    }
}

/// Build the grounded-context block injected for the model.
fn build_context_block(sources: &[RecalledSource]) -> String {
    let mut block = String::from(
        "# Recalled context from past sessions\n\
         The following excerpts come from this user's earlier sessions and may be relevant. \
         Use them to ground your answer when appropriate, and cite the session title when you rely \
         on one. If they are not relevant, ignore them.\n\n",
    );
    for (i, source) in sources.iter().enumerate() {
        block.push_str(&format!(
            "{}. \"{}\" ({})\n   {}\n\n",
            i + 1,
            source.title,
            source.date,
            source.snippet
        ));
    }
    block
}

fn build_user_summary(sources: &[RecalledSource]) -> String {
    let citations = sources
        .iter()
        .map(|s| format!("\"{}\" ({})", s.title, s.date))
        .collect::<Vec<_>>()
        .join(", ");
    format!("🧠 Recalled from past sessions: {citations}")
}

/// Run the recall step for a user message. Returns `None` when nothing relevant
/// is found or recall isn't warranted.
pub async fn recall_for_message(
    session_manager: &SessionManager,
    provider: &Arc<dyn Provider>,
    current_session_id: &str,
    current_session_type: Option<SessionType>,
    user_text: &str,
) -> Option<RecallOutcome> {
    let keywords = extract_keywords(provider, user_text).await?;

    let results = session_manager
        .search_chat_history(
            &keywords,
            Some(MAX_RECALLED_SESSIONS),
            None,
            None,
            Some(current_session_id.to_string()),
            search_session_types(current_session_type),
        )
        .await
        .ok()?;

    if results.results.is_empty() {
        return None;
    }

    let sources: Vec<RecalledSource> = results
        .results
        .iter()
        .take(MAX_RECALLED_SESSIONS)
        .map(|r| {
            let snippet = best_snippet(r);
            RecalledSource {
                session_id: r.session_id.clone(),
                title: session_title(&r.session_description, &snippet),
                date: r.last_activity.format("%Y-%m-%d").to_string(),
                snippet,
            }
        })
        .collect();

    if sources.is_empty() {
        return None;
    }

    let context_block = build_context_block(&sources);
    let user_summary = build_user_summary(&sources);

    Some(RecallOutcome {
        sources,
        context_block,
        user_summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn source(title: &str, date: &str) -> RecalledSource {
        RecalledSource {
            session_id: "s1".to_string(),
            title: title.to_string(),
            date: date.to_string(),
            snippet: "front 27 rear 29 cold".to_string(),
        }
    }

    #[test]
    fn heuristic_triggers_only_on_memory_phrases() {
        assert!(heuristic_keywords("remind me what tyre pressure to use").is_some());
        assert!(heuristic_keywords("we decided on the database schema").is_some());
        assert!(heuristic_keywords("write a new function please").is_none());
        assert!(heuristic_keywords("hello there").is_none());
    }

    #[test]
    fn truncate_collapses_whitespace_and_caps() {
        assert_eq!(truncate_chars("a   b\n c", 100), "a b c");
        assert_eq!(truncate_chars("hello world", 5), "hello…");
    }

    #[test]
    fn context_block_lists_titles_and_dates() {
        let sources = vec![source("Tyre Pressure 981", "2026-05-02")];
        let block = build_context_block(&sources);
        assert!(block.contains("Recalled context from past sessions"));
        assert!(block.contains("\"Tyre Pressure 981\" (2026-05-02)"));
        assert!(block.contains("front 27 rear 29 cold"));
    }

    #[test]
    fn user_summary_cites_sources() {
        let sources = vec![
            source("Tyre Pressure 981", "2026-05-02"),
            source("981 Aero Effects", "2026-06-13"),
        ];
        let summary = build_user_summary(&sources);
        assert!(summary.starts_with("🧠 Recalled from past sessions:"));
        assert!(summary.contains("\"Tyre Pressure 981\" (2026-05-02)"));
        assert!(summary.contains("\"981 Aero Effects\" (2026-06-13)"));
    }

    #[test]
    fn session_title_falls_back_to_snippet_when_unnamed() {
        assert_eq!(
            session_title("Tyre Pressure 981", "anything"),
            "Tyre Pressure 981"
        );
        assert_eq!(
            session_title("  ", "front 27 rear 29 cold"),
            "front 27 rear 29 cold"
        );
        assert_eq!(session_title("", ""), "Untitled session");
    }

    #[test]
    fn best_snippet_prefers_user_message() {
        let result = ChatRecallResult {
            session_id: "s".to_string(),
            session_description: "d".to_string(),
            session_working_dir: "/".to_string(),
            last_activity: Utc::now(),
            total_messages_in_session: 2,
            messages: vec![
                ChatRecallMessage {
                    role: "assistant".to_string(),
                    content: "the answer is 42".to_string(),
                    timestamp: Utc::now(),
                },
                ChatRecallMessage {
                    role: "user".to_string(),
                    content: "what is the pressure".to_string(),
                    timestamp: Utc::now(),
                },
            ],
        };
        assert_eq!(best_snippet(&result), "what is the pressure");
    }
}
