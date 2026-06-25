use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::StreamExt;
use rmcp::model::{Role, Tool};

use crate::agents::state_machine::operation::{Emitter, Operation, TurnOutcome};
use crate::agents::AgentEvent;
use crate::conversation::message::Message;
use crate::conversation::Conversation;
use crate::providers::base::Provider;
use crate::session::Session;
use goose_providers::errors::ProviderError;
use goose_providers::model::ModelConfig;

/// Calls the LLM when the last message in the conversation is from the user.
pub struct LlmOperation {
    provider: Arc<dyn Provider>,
    model_config: ModelConfig,
    system_prompt: String,
    tools: Vec<Tool>,
}

impl LlmOperation {
    pub fn new(
        provider: Arc<dyn Provider>,
        model_config: ModelConfig,
        system_prompt: String,
        tools: Vec<Tool>,
    ) -> Self {
        Self {
            provider,
            model_config,
            system_prompt,
            tools,
        }
    }

    async fn error_outcome(&self, err: &ProviderError, emit: &Emitter) -> TurnOutcome {
        #[cfg(feature = "telemetry")]
        crate::posthog::emit_error(err.telemetry_type(), &err.to_string());
        tracing::error!("LLM provider error: {err}");
        let message = Message::from_provider_error(err);
        emit.emit(AgentEvent::Message(message.clone())).await;
        vec![message.into()]
    }
}

#[async_trait]
impl Operation for LlmOperation {
    fn name(&self) -> &'static str {
        "llm"
    }

    fn applies(&self, session: &Session) -> bool {
        matches!(
            session
                .conversation
                .as_ref()
                .and_then(|c| c.last())
                .map(|m| &m.role),
            Some(Role::User)
        )
    }

    async fn run(&self, session: &Session, emit: Emitter) -> Result<TurnOutcome> {
        let conversation = session
            .conversation
            .as_ref()
            .ok_or_else(|| anyhow!("LlmOperation::run with no conversation"))?;

        let messages_for_provider: Vec<_> = conversation
            .messages()
            .iter()
            .filter(|m| m.is_agent_visible())
            .map(|m| m.agent_visible_content())
            .collect();

        let stream = self
            .provider
            .stream(
                &self.model_config,
                &session.id,
                &self.system_prompt,
                &messages_for_provider,
                &self.tools,
            )
            .await;

        let mut stream = match stream {
            Ok(stream) => stream,
            Err(err) => return Ok(self.error_outcome(&err, &emit).await),
        };

        // Conversation::push handles merge logic — coalescing text, merging
        // thinking blocks by signature, deduping by message id, forwarding
        // inference metadata to the right prior message.
        let mut accumulator = Conversation::empty();
        loop {
            tokio::select! {
                biased;
                _ = emit.cancelled() => break,
                next = stream.next() => {
                    let Some(result) = next else { break };
                    let (msg_opt, _usage_opt) = match result {
                        Ok(chunk) => chunk,
                        // A mid-stream provider error: discard the partial
                        // assistant turn and append a tagged error message so a
                        // recovery op (or ExitOnError) handles it on the next
                        // iteration. The conversation never keeps a half-turn.
                        Err(err) => return Ok(self.error_outcome(&err, &emit).await),
                    };
                    if let Some(chunk) = msg_opt {
                        emit.emit(AgentEvent::Message(chunk.clone())).await;
                        accumulator.push(chunk);
                    }
                }
            }
        }

        Ok(accumulator.into_iter().map(Into::into).collect())
    }
}
