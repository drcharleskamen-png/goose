use goose_providers::conversation::Conversation;
use rmcp::model::Role;

const ANCHOR_MARKER: &str = ">>>";
const RECENT_STATE_EVENTS: usize = 6;
const SNIPPET_CHARS: usize = 220;

/// Render `conversation` into the `user:/assistant:` form used at training
/// time. The most recent *real* user message is the anchor, marked with `>>>`;
/// a compact tail of recent agent state follows so the score can move as a
/// trajectory degrades or recovers.
///
/// goose injects narration as user-role messages (e.g. "A grep search found
/// that...") flagged `user_visible: false`. Those are not user intent, so we
/// anchor on the most recent user-visible user turn and treat the rest as
/// state. This is the native advantage over an HTTP proxy, which loses that
/// flag and has to guess with regex.
///
/// Returns `None` if no user message exists. We don't enforce a token budget
/// here — encoders truncate to their max sequence length, and the anchor is at
/// the front so truncation drops the oldest state first.
pub fn render_for_routing(conversation: &Conversation) -> Option<String> {
    let messages = conversation.messages();

    let anchor_idx = messages
        .iter()
        .enumerate()
        .rev()
        .find(|(_, m)| matches!(m.role, Role::User) && m.metadata.user_visible)
        .or_else(|| {
            messages
                .iter()
                .enumerate()
                .rev()
                .find(|(_, m)| matches!(m.role, Role::User))
        })
        .map(|(i, _)| i)?;

    let anchor_text = collapse(&messages[anchor_idx].as_concat_text());
    if anchor_text.is_empty() {
        return None;
    }
    let anchor_line = format!(
        "{} user: {}",
        ANCHOR_MARKER,
        clip(&anchor_text, SNIPPET_CHARS)
    );

    let mut tail = Vec::new();
    for msg in messages.iter().skip(anchor_idx + 1) {
        let label = match msg.role {
            Role::Assistant if msg.is_tool_call() => "tool",
            Role::Assistant => "assistant",
            Role::User if msg.is_tool_response() => "tool",
            Role::User => "user",
        };
        let text = clip(&collapse(&msg.as_concat_text()), SNIPPET_CHARS);
        if !text.is_empty() {
            tail.push(format!("{}: {}", label, text));
        }
    }

    if tail.is_empty() {
        return Some(anchor_line);
    }

    let start = tail.len().saturating_sub(RECENT_STATE_EVENTS);
    let mut out = String::with_capacity(anchor_line.len() + 64);
    out.push_str(&anchor_line);
    out.push_str("\n--- recent agent state ---");
    for line in &tail[start..] {
        out.push('\n');
        out.push_str(line);
    }
    Some(out)
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clip(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut s: String = text.chars().take(limit.saturating_sub(1)).collect();
    s.push('…');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use goose_providers::conversation::message::Message;

    fn convo(msgs: Vec<Message>) -> Conversation {
        Conversation::new_unvalidated(msgs)
    }

    #[test]
    fn render_empty_returns_none() {
        let c = convo(vec![]);
        assert!(render_for_routing(&c).is_none());
    }

    #[test]
    fn render_single_user_message() {
        let c = convo(vec![Message::user().with_text("hi there")]);
        let r = render_for_routing(&c).expect("some");
        assert_eq!(r, ">>> user: hi there");
    }

    #[test]
    fn render_anchors_last_user_with_state_tail() {
        let c = convo(vec![
            Message::user().with_text("what is 2+2"),
            Message::assistant().with_text("4"),
            Message::user().with_text("now squared"),
        ]);
        let r = render_for_routing(&c).expect("some");
        assert!(r.starts_with(">>> user: now squared"));
        assert!(!r.contains("what is 2+2"), "older context dropped: {r}");
    }

    #[test]
    fn render_anchors_on_real_user_skipping_narration() {
        let narration = Message::user()
            .with_text("A grep search found that the config files differ")
            .agent_only();
        let c = convo(vec![
            Message::user().with_text("fix the failing build"),
            Message::assistant().with_text("looking into it"),
            narration,
        ]);
        let r = render_for_routing(&c).expect("some");
        assert!(
            r.starts_with(">>> user: fix the failing build"),
            "should anchor real intent, not narration: {r}"
        );
        assert!(
            r.contains("--- recent agent state ---"),
            "narration kept as state tail: {r}"
        );
        assert!(r.contains("A grep search found"));
    }

    #[test]
    fn render_collapses_whitespace() {
        let c = convo(vec![Message::user().with_text("real    question\n\nhere")]);
        let r = render_for_routing(&c).expect("some");
        assert_eq!(r, ">>> user: real question here");
    }
}
