use anyhow::Result;
use futures::StreamExt;
use rmcp::model::Role;

use crate::agents::state_machine::test_helpers::{
    tool_response_text, ScriptedProvider, Step, TestHarness,
};
use crate::agents::tool_execution::DECLINED_RESPONSE;
use crate::agents::types::SessionConfig;
use crate::agents::{state_machine, AgentEvent};
use crate::config::GooseMode;
use crate::conversation::message::{ActionRequiredData, Message, MessageContent};
use crate::permission::permission_confirmation::PrincipalType;
use crate::permission::{Permission, PermissionConfirmation};
use std::sync::Arc;

#[tokio::test]
async fn llm_requests_tool_then_replies() -> Result<()> {
    let harness = TestHarness::with_steps([
        Step::ToolCall {
            id: "call_1".to_string(),
            name: "test__echo".to_string(),
            args: serde_json::json!({ "x": 1 }),
        },
        Step::Text("all done".to_string()),
    ])
    .await
    .with_default_extension()
    .await;

    let messages = harness.run("use the echo tool", 10).await?;

    // emitted: assistant(tool req) + user(tool resp) + assistant(text)
    assert_eq!(messages.len(), 3, "events: {messages:#?}");
    assert_eq!(messages[0].role, Role::Assistant);
    assert!(messages[0].is_tool_call());
    assert_eq!(messages[1].role, Role::User);
    assert!(messages[1].is_tool_response());
    assert_eq!(messages[2].role, Role::Assistant);

    // tool actually ran: echo returned the args as JSON text
    let resp_text = tool_response_text(&messages[1]);
    assert!(resp_text.contains("\"x\":1"), "tool response: {resp_text}");

    // provider was called twice (tool turn + final text turn)
    assert_eq!(harness.provider.call_count(), 2);

    // persisted conversation matches what was emitted (prompt + 3 above)
    let persisted = harness.persisted_messages().await?;
    assert_eq!(persisted.len(), 4);
    assert_eq!(persisted[0].role, Role::User);

    Ok(())
}

#[tokio::test]
async fn stops_at_max_turns() -> Result<()> {
    // The provider never stops on its own — every turn calls a tool, whose
    // response re-triggers the LLM. Only the max-turns op can halt the loop.
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let provider = Arc::new(ScriptedProvider::from_fn(move |_messages, _tools| {
        let n = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        vec![Message::assistant().with_tool_request(
            format!("call_{n}"),
            Ok(rmcp::model::CallToolRequestParams::new("test__echo")
                .with_arguments(serde_json::Map::new())),
        )]
    }));
    let harness = TestHarness::with_provider(provider)
        .await
        .with_default_extension()
        .await;

    let messages = harness.run("keep going", 3).await?;

    // 3 LLM turns, then the max-turns op halts before a 4th.
    assert_eq!(harness.provider.call_count(), 3);

    let limit = messages.last().expect("at least one message");
    assert_eq!(limit.role, Role::Assistant);
    assert!(
        limit.as_concat_text().contains("maximum number of actions"),
        "last message: {limit:#?}"
    );

    // The 3 tool-calling turns are persisted; the limit message is not.
    let persisted = harness.persisted_messages().await?;
    let tool_call_turns = persisted.iter().filter(|m| m.is_tool_call()).count();
    assert_eq!(tool_call_turns, 3);

    Ok(())
}

#[tokio::test]
async fn approve_mode_waits_for_tool_confirmation_before_execution() -> Result<()> {
    let harness = TestHarness::with_steps([
        Step::ToolCall {
            id: "call_1".to_string(),
            name: "test__echo".to_string(),
            args: serde_json::json!({ "x": 1 }),
        },
        Step::Text("done".to_string()),
    ])
    .await
    .with_default_extension()
    .await
    .with_goose_mode(GooseMode::Approve)
    .await;

    let stream = state_machine::reply(
        &harness.agent,
        Message::user().with_text("use the echo tool"),
        SessionConfig {
            id: harness.session_id.clone(),
            schedule_id: None,
            max_turns: Some(10),
            retry_config: None,
        },
        None,
    )
    .await?;
    tokio::pin!(stream);

    let mut messages = Vec::new();
    let mut saw_confirmation = false;
    while let Some(event) = stream.next().await {
        let event = event?;
        if let AgentEvent::Message(message) = &event {
            if message.content.iter().any(|content| {
                matches!(
                    content,
                    MessageContent::ActionRequired(action)
                        if matches!(
                            action.data,
                            ActionRequiredData::ToolConfirmation { ref id, .. } if id == "call_1"
                        )
                )
            }) {
                saw_confirmation = true;
                harness
                    .agent
                    .handle_confirmation(
                        "call_1".to_string(),
                        PermissionConfirmation {
                            principal_type: PrincipalType::Tool,
                            permission: Permission::AllowOnce,
                        },
                    )
                    .await;
            }
            messages.push(message.clone());
        }
    }

    assert!(saw_confirmation, "messages: {messages:#?}");
    assert_eq!(harness.provider.call_count(), 2);
    assert!(messages.iter().any(|m| {
        m.role == Role::User && m.is_tool_response() && tool_response_text(m).contains("\"x\":1")
    }));

    Ok(())
}

#[tokio::test]
async fn denied_tool_confirmation_becomes_tool_response() -> Result<()> {
    let harness = TestHarness::with_steps([
        Step::ToolCall {
            id: "call_1".to_string(),
            name: "test__echo".to_string(),
            args: serde_json::json!({ "x": 1 }),
        },
        Step::Text("done".to_string()),
    ])
    .await
    .with_default_extension()
    .await
    .with_goose_mode(GooseMode::Approve)
    .await;

    let stream = state_machine::reply(
        &harness.agent,
        Message::user().with_text("use the echo tool"),
        SessionConfig {
            id: harness.session_id.clone(),
            schedule_id: None,
            max_turns: Some(10),
            retry_config: None,
        },
        None,
    )
    .await?;
    tokio::pin!(stream);

    let mut messages = Vec::new();
    while let Some(event) = stream.next().await {
        let event = event?;
        if let AgentEvent::Message(message) = &event {
            if message.content.iter().any(|content| {
                matches!(
                    content,
                    MessageContent::ActionRequired(action)
                        if matches!(
                            action.data,
                            ActionRequiredData::ToolConfirmation { ref id, .. } if id == "call_1"
                        )
                )
            }) {
                harness
                    .agent
                    .handle_confirmation(
                        "call_1".to_string(),
                        PermissionConfirmation {
                            principal_type: PrincipalType::Tool,
                            permission: Permission::DenyOnce,
                        },
                    )
                    .await;
            }
            messages.push(message.clone());
        }
    }

    assert_eq!(harness.provider.call_count(), 2);
    assert!(messages.iter().any(|m| {
        m.role == Role::User
            && m.is_tool_response()
            && tool_response_text(m).contains(DECLINED_RESPONSE)
    }));

    Ok(())
}

#[tokio::test]
async fn compacts_when_over_token_threshold() -> Result<()> {
    // Every provider call (the compaction summary and the post-compaction LLM
    // turn) returns plain text, so the loop ends after one real turn.
    let provider = Arc::new(ScriptedProvider::from_fn(|_messages, _tools| {
        vec![Message::assistant().with_text("ok")]
    }));
    let harness = TestHarness::with_provider(provider).await;

    // 128k context * 0.8 threshold = 102_400; push well past it.
    harness.set_total_tokens(120_000).await;

    let events = harness.run_events("hello", 10).await?;

    // Compaction replaced the conversation exactly once.
    let replaced = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::HistoryReplaced(_)))
        .count();
    assert_eq!(replaced, 1, "events: {events:#?}");

    // The "Performing auto-compaction" notice was emitted.
    use crate::conversation::message::MessageContent;
    let saw_notice = events.iter().any(|e| {
        match e {
        AgentEvent::Message(m) => m.content.iter().any(|c| {
            matches!(c, MessageContent::SystemNotification(s) if s.msg.contains("auto-compaction"))
        }),
        _ => false,
    }
    });
    assert!(saw_notice, "events: {events:#?}");

    // Provider was called for the summary and then the post-compaction turn.
    assert_eq!(harness.provider.call_count(), 2);

    // The token total was cleared so compaction doesn't re-trigger.
    let reloaded = harness.reload().await?;
    assert!(reloaded.usage.total_tokens.is_none());

    Ok(())
}

#[tokio::test]
async fn provider_error_is_persisted_and_yields() -> Result<()> {
    use crate::conversation::message::MessageErrorKind;
    use goose_providers::errors::ProviderError;

    let provider = Arc::new(ScriptedProvider::from_steps([Step::Error(
        ProviderError::ServerError("boom".to_string()),
    )]));
    let harness = TestHarness::with_provider(provider).await;

    let events = harness.run_events("hello", 10).await?;

    // The error surfaced as a message event (replacing the old notification).
    let saw_error_event = events.iter().any(|e| {
        matches!(
            e,
            AgentEvent::Message(m) if m.error_kind() == Some(MessageErrorKind::Other)
        )
    });
    assert!(saw_error_event, "events: {events:#?}");

    // It is durable conversation state, tagged, user-visible, agent-invisible.
    let persisted = harness.persisted_messages().await?;
    let last = persisted.last().expect("a persisted message");
    assert_eq!(last.error_kind(), Some(MessageErrorKind::Other));
    assert!(last.is_user_visible());
    assert!(!last.is_agent_visible());

    // The provider was called exactly once: ExitOnError yielded, no retry.
    assert_eq!(harness.provider.call_count(), 1);

    Ok(())
}

#[tokio::test]
async fn slash_command_yields_without_calling_provider() -> Result<()> {
    let provider = Arc::new(ScriptedProvider::from_steps([Step::Text(
        "should not run".to_string(),
    )]));
    let harness = TestHarness::with_provider(provider).await;

    let messages = harness.run("/status", 10).await?;

    assert_eq!(harness.provider.call_count(), 0);
    assert_eq!(messages.len(), 2, "events: {messages:#?}");
    assert_eq!(messages[0].role, Role::User);
    assert_eq!(messages[0].as_concat_text(), "/status");
    assert_eq!(messages[1].role, Role::Assistant);
    assert!(
        messages[1].as_concat_text().contains("Provider:"),
        "response: {:#?}",
        messages[1]
    );

    let persisted = harness.persisted_messages().await?;
    assert_eq!(persisted.len(), 2);
    assert!(persisted.iter().all(|m| m.is_user_visible()));
    assert!(persisted.iter().all(|m| !m.is_agent_visible()));

    Ok(())
}

#[tokio::test]
async fn unknown_slash_text_falls_through_to_provider() -> Result<()> {
    let harness = TestHarness::with_steps([Step::Text("saw it".to_string())]).await;

    let messages = harness.run("/not-a-command", 10).await?;

    assert_eq!(harness.provider.call_count(), 1);
    assert_eq!(messages.len(), 1, "events: {messages:#?}");
    assert_eq!(messages[0].as_concat_text(), "saw it");

    let persisted = harness.persisted_messages().await?;
    assert_eq!(persisted.len(), 2);
    assert_eq!(persisted[0].as_concat_text(), "/not-a-command");
    assert!(persisted[0].is_user_visible());
    assert!(persisted[0].is_agent_visible());

    Ok(())
}

#[tokio::test]
async fn goal_slash_command_starts_turn_with_hidden_kickoff() -> Result<()> {
    let harness = TestHarness::with_steps([Step::Text("working on it".to_string())]).await;

    let messages = harness.run("/goal finish the migration", 10).await?;

    assert_eq!(harness.provider.call_count(), 1);
    assert_eq!(messages.len(), 3, "events: {messages:#?}");
    assert_eq!(messages[0].role, Role::User);
    assert_eq!(messages[1].role, Role::Assistant);
    assert_eq!(messages[2].as_concat_text(), "working on it");

    let persisted = harness.persisted_messages().await?;
    assert_eq!(persisted.len(), 4);
    assert_eq!(persisted[0].as_concat_text(), "/goal finish the migration");
    assert!(persisted[0].is_user_visible());
    assert!(!persisted[0].is_agent_visible());
    assert!(persisted[1].is_user_visible());
    assert!(!persisted[1].is_agent_visible());
    assert!(persisted[2]
        .as_concat_text()
        .contains("finish the migration"));
    assert!(!persisted[2].is_user_visible());
    assert!(persisted[2].is_agent_visible());
    assert_eq!(persisted[3].as_concat_text(), "working on it");

    Ok(())
}

#[tokio::test]
async fn history_slash_command_replaces_history_and_yields() -> Result<()> {
    let provider = Arc::new(ScriptedProvider::from_steps([Step::Text(
        "should not run".to_string(),
    )]));
    let harness = TestHarness::with_provider(provider).await;

    let events = harness.run_events("/clear", 10).await?;

    assert_eq!(harness.provider.call_count(), 0);
    let replaced = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::HistoryReplaced(_)))
        .count();
    assert_eq!(replaced, 1, "events: {events:#?}");

    let messages: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::Message(m) => Some(m),
            _ => None,
        })
        .collect();
    assert_eq!(messages.len(), 2, "events: {events:#?}");
    assert_eq!(messages[0].role, Role::User);
    assert_eq!(messages[0].as_concat_text(), "/clear");
    assert_eq!(messages[1].role, Role::Assistant);

    let persisted = harness.persisted_messages().await?;
    assert_eq!(persisted.len(), 2);
    assert!(persisted.iter().all(|m| m.is_user_visible()));
    assert!(persisted.iter().all(|m| !m.is_agent_visible()));

    Ok(())
}

#[tokio::test]
async fn context_length_error_triggers_compaction_recovery() -> Result<()> {
    use goose_providers::errors::ProviderError;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // First LLM call blows the context; after compaction replaces the
    // conversation, the retried call succeeds with plain text.
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_fn = calls.clone();
    let provider = Arc::new(ScriptedProvider::from_fn_result(
        move |_messages, _tools| {
            match calls_for_fn.fetch_add(1, Ordering::SeqCst) {
                // call 0: the failing turn
                0 => Err(ProviderError::ContextLengthExceeded("too long".to_string())),
                // call 1: the compaction summary
                // call 2: the retried turn
                _ => Ok(vec![Message::assistant().with_text("recovered")]),
            }
        },
    ));
    let harness = TestHarness::with_provider(provider).await;

    let events = harness.run_events("hello", 10).await?;

    // Compaction replaced the conversation as part of recovery.
    let replaced = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::HistoryReplaced(_)))
        .count();
    assert_eq!(replaced, 1, "events: {events:#?}");

    // The turn ultimately succeeded; no error message lingers on the tail.
    let persisted = harness.persisted_messages().await?;
    let last = persisted.last().expect("a persisted message");
    assert!(last.error_kind().is_none(), "tail still an error: {last:?}");

    // Failing turn + compaction summary + retried turn = three provider calls.
    assert_eq!(calls.load(Ordering::SeqCst), 3);

    Ok(())
}
