use anyhow::{anyhow, Result};
use async_trait::async_trait;
use rmcp::model::Role;

use crate::agents::execute_commands::{
    command_starts_turn, is_known_slash_command, parse_slash_command, COMPACT_TRIGGERS,
};
use crate::agents::state_machine::operation::{Emitter, Operation, TurnEffect, TurnOutcome};
use crate::agents::{Agent, AgentEvent};
use crate::conversation::message::Message;
use crate::session::Session;

pub struct SlashCommandOperation<'a> {
    agent: &'a Agent,
}

impl<'a> SlashCommandOperation<'a> {
    pub fn new(agent: &'a Agent) -> Self {
        Self { agent }
    }
}

#[async_trait]
impl Operation for SlashCommandOperation<'_> {
    fn name(&self) -> &'static str {
        "slash_command"
    }

    fn applies(&self, session: &Session) -> bool {
        let Some(message) = session.conversation.as_ref().and_then(|c| c.last()) else {
            return false;
        };

        message.role == Role::User
            && message.is_agent_visible()
            && is_known_slash_command(
                &message.as_concat_text(),
                Some(session.working_dir.as_path()),
            )
    }

    async fn run(&self, session: &Session, emit: Emitter) -> Result<TurnOutcome> {
        let user_message = session
            .conversation
            .as_ref()
            .and_then(|c| c.last())
            .cloned()
            .ok_or_else(|| anyhow!("Slash command operation ran without a conversation tail"))?;
        let message_id = user_message
            .id
            .clone()
            .ok_or_else(|| anyhow!("Persisted slash command message has no id"))?;
        let message_text = user_message.as_concat_text();

        let command_result = self.agent.execute_command(&message_text, &session.id).await;

        match command_result {
            Err(e) => {
                let error_message = Message::assistant()
                    .with_text(e.to_string())
                    .with_visibility(true, false);
                emit.emit(AgentEvent::Message(error_message.clone())).await;
                Ok(vec![
                    TurnEffect::SetMessageVisibility {
                        message_id,
                        user_visible: true,
                        agent_visible: false,
                    },
                    error_message.into(),
                    TurnEffect::YieldToClient,
                ])
            }
            Ok(Some(response)) if response.role == Role::Assistant => {
                let mut effects = Vec::new();
                let user_only_command = user_message.with_visibility(true, false);
                let user_only_response = response.with_visibility(true, false);

                if modifies_history(&message_text) {
                    effects.push(user_only_command.clone().into());
                    effects.push(user_only_response.clone().into());
                    effects.push(TurnEffect::EmitCurrentHistoryReplaced);
                } else {
                    effects.push(TurnEffect::SetMessageVisibility {
                        message_id,
                        user_visible: true,
                        agent_visible: false,
                    });
                    effects.push(user_only_response.clone().into());
                }

                emit.emit(AgentEvent::Message(user_only_command)).await;
                emit.emit(AgentEvent::Message(user_only_response)).await;

                if command_starts_turn(&message_text) {
                    let goal_text = parse_slash_command(&message_text)
                        .map(|parsed| parsed.params_str.to_string())
                        .unwrap_or_default();
                    effects.push(
                        Message::user()
                            .with_text(format!(
                                "Start working toward this goal now:\n\n**Goal:** {goal_text}"
                            ))
                            .with_visibility(false, true)
                            .into(),
                    );
                } else {
                    effects.push(TurnEffect::YieldToClient);
                }

                Ok(effects)
            }
            Ok(Some(resolved_message)) => Ok(vec![
                TurnEffect::SetMessageVisibility {
                    message_id,
                    user_visible: true,
                    agent_visible: false,
                },
                resolved_message.with_visibility(false, true).into(),
            ]),
            Ok(None) => Ok(vec![]),
        }
    }
}

fn modifies_history(message_text: &str) -> bool {
    let trimmed = message_text.trim();
    COMPACT_TRIGGERS.contains(&trimmed) || trimmed == "/clear"
}
