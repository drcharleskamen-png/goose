use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::StreamExt;
use rmcp::model::{CallToolResult, Content, ErrorCode, ErrorData, Role};

use crate::agents::agent::{tool_stream, ToolStreamItem};
use crate::agents::extension_manager::ExtensionManager;
use crate::agents::state_machine::operation::{Emitter, Operation, TurnOutcome};
use crate::agents::tool_execution::{ToolCallContext, ToolCallResult};
use crate::agents::AgentEvent;
use crate::conversation::message::{Message, MessageContent, ToolRequest};
use crate::session::Session;

/// Executes pending tool requests: when the last message is an assistant
/// message carrying tool requests that have not yet been answered, dispatch
/// each one through the extension manager and append a single message with the
/// collected responses.
///
/// Scoped to ordinary extension tools. Approval, frontend tools (which yield to
/// the client), platform tools, and hooks are handled elsewhere.
pub struct ToolExecutionOperation {
    extension_manager: Arc<ExtensionManager>,
}

impl ToolExecutionOperation {
    pub fn new(extension_manager: Arc<ExtensionManager>) -> Self {
        Self { extension_manager }
    }
}

fn pending_tool_requests(session: &Session) -> Vec<ToolRequest> {
    let Some(last) = session
        .conversation
        .as_ref()
        .and_then(|c| c.messages().last())
    else {
        return Vec::new();
    };

    if last.role != Role::Assistant {
        return Vec::new();
    }

    last.content
        .iter()
        .filter_map(|c| match c {
            MessageContent::ToolRequest(req) if req.tool_call.is_ok() => Some(req.clone()),
            _ => None,
        })
        .collect()
}

#[async_trait]
impl Operation for ToolExecutionOperation {
    fn name(&self) -> &'static str {
        "tool_execution"
    }

    fn applies(&self, session: &Session) -> bool {
        !pending_tool_requests(session).is_empty()
    }

    async fn run(&self, session: &Session, emit: Emitter) -> Result<TurnOutcome> {
        let requests = pending_tool_requests(session);
        if requests.is_empty() {
            return Err(anyhow!(
                "ToolExecutionOperation::run with no pending requests"
            ));
        }

        let mut tool_streams = Vec::new();
        for request in &requests {
            let tool_call = request
                .tool_call
                .clone()
                .map_err(|e| anyhow!("tool call could not be parsed: {e}"))?;
            let ctx = ToolCallContext::new(
                session.id.clone(),
                Some(session.working_dir.clone()),
                Some(request.id.clone()),
            );
            let result = self
                .extension_manager
                .dispatch_tool_call(&ctx, tool_call, emit.cancel_token().clone())
                .await
                .unwrap_or_else(|e| {
                    let error_data = e.downcast::<ErrorData>().unwrap_or_else(|e| {
                        ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None)
                    });
                    ToolCallResult::from(Err(error_data))
                });

            let req_id = request.id.clone();
            let stream = tool_stream(
                result
                    .notification_stream
                    .unwrap_or_else(|| Box::new(futures::stream::empty())),
                result
                    .action_required_stream
                    .unwrap_or_else(|| Box::new(futures::stream::empty())),
                result.result,
            )
            .map(move |item| (req_id.clone(), item));
            tool_streams.push(stream);
        }

        let mut combined = futures::stream::select_all(tool_streams);
        let mut response = Message::user().with_generated_id();

        loop {
            tokio::select! {
                biased;
                _ = emit.cancelled() => break,
                item = combined.next() => {
                    let Some((request_id, item)) = item else { break };
                    match item {
                        ToolStreamItem::Result(output) => {
                            let metadata = requests
                                .iter()
                                .find(|r| r.id == request_id)
                                .and_then(|r| r.metadata.as_ref());
                            response.add_tool_response_with_metadata(request_id, output, metadata);
                        }
                        ToolStreamItem::Message(msg) => {
                            emit.emit(AgentEvent::McpNotification((request_id, msg)))
                                .await;
                        }
                        ToolStreamItem::ActionRequired(mut msg) => {
                            if msg.id.is_none() {
                                msg = msg.with_generated_id();
                            }
                            emit.emit(AgentEvent::Message(msg)).await;
                        }
                    }
                }
            }
        }

        let answered: HashSet<String> = response
            .get_tool_response_ids()
            .into_iter()
            .map(str::to_string)
            .collect();
        for request in &requests {
            if !answered.contains(request.id.as_str()) {
                response.add_tool_response_with_metadata(
                    request.id.clone(),
                    Ok(CallToolResult::error(vec![Content::text(
                        "Tool call was interrupted before completing",
                    )])),
                    request.metadata.as_ref(),
                );
            }
        }

        emit.emit(AgentEvent::Message(response.clone())).await;
        Ok(TurnOutcome::AppendMessages(vec![response]))
    }
}
