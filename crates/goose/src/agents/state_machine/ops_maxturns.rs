use anyhow::Result;
use async_trait::async_trait;
use rmcp::model::Role;

use crate::agents::state_machine::operation::{Emitter, Operation, TurnOutcome};
use crate::agents::AgentEvent;
use crate::conversation::message::Message;
use crate::session::Session;

/// Stops the loop once the agent has taken `max_turns` LLM turns in response to
/// a single user prompt. A "turn" is one assistant message; the current request
/// starts at the last genuine user message (a prompt, not a tool response).
pub struct MaxTurnsOperation {
    max_turns: u32,
}

impl MaxTurnsOperation {
    pub fn new(max_turns: u32) -> Self {
        Self { max_turns }
    }
}

fn turns_taken_this_request(session: &Session) -> u32 {
    let Some(conversation) = session.conversation.as_ref() else {
        return 0;
    };
    let mut turns = 0u32;
    for message in conversation.messages().iter().rev() {
        if message.role == Role::User && !message.is_tool_response() {
            break;
        }
        if message.role == Role::Assistant {
            turns += 1;
        }
    }
    turns
}

#[async_trait]
impl Operation for MaxTurnsOperation {
    fn name(&self) -> &'static str {
        "max_turns"
    }

    fn applies(&self, session: &Session) -> bool {
        turns_taken_this_request(session) >= self.max_turns
    }

    async fn run(&self, _session: &Session, emit: Emitter) -> Result<TurnOutcome> {
        let message = Message::assistant().with_text(
            "I've reached the maximum number of actions I can do without user input. \
             Would you like me to continue?",
        );
        emit.emit(AgentEvent::Message(message)).await;
        Ok(TurnOutcome::YieldToClient)
    }
}
