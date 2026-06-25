use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::agents::AgentEvent;
use crate::conversation::message::Message;
use crate::conversation::Conversation;
use crate::session::Session;

/// One step in the agent loop. The first op whose `applies` returns true
/// gets to `run` — it streams events via the emitter and returns an outcome.
#[async_trait]
pub trait Operation: Send + Sync {
    fn name(&self) -> &'static str;

    fn applies(&self, session: &Session) -> bool;

    async fn run(&self, session: &Session, emit: Emitter) -> Result<TurnOutcome>;
}

pub type TurnOutcome = Vec<TurnEffect>;

/// One action the machine applies after an operation finishes.
pub enum TurnEffect {
    AppendMessage(Message),
    ReplaceConversation(Conversation),
    SetMessageVisibility {
        message_id: String,
        user_visible: bool,
        agent_visible: bool,
    },
    EmitCurrentHistoryReplaced,
    YieldToClient,
}

impl From<Message> for TurnEffect {
    fn from(message: Message) -> Self {
        TurnEffect::AppendMessage(message)
    }
}

impl From<Conversation> for TurnEffect {
    fn from(conversation: Conversation) -> Self {
        TurnEffect::ReplaceConversation(conversation)
    }
}

/// An op's handle to the machine: emit events the client should see, and
/// observe cancellation. Long-running ops `select!` on [`Emitter::cancelled`];
/// short ops can ignore it entirely.
pub struct Emitter {
    tx: mpsc::Sender<AgentEvent>,
    cancel: CancellationToken,
}

impl Emitter {
    pub fn new(tx: mpsc::Sender<AgentEvent>, cancel: CancellationToken) -> Self {
        Self { tx, cancel }
    }

    /// Drops silently if the receiver is gone (caller cancelled the stream).
    pub async fn emit(&self, event: AgentEvent) {
        let _ = self.tx.send(event).await;
    }

    /// The machine's cancellation token. Ops use it as they need — poll it,
    /// `select!` on it, or hand it to work that observes cancellation itself
    /// (e.g. tool dispatch).
    pub fn cancel_token(&self) -> &CancellationToken {
        &self.cancel
    }

    /// Resolves when cancellation is requested. Convenience for `select!` arms.
    pub async fn cancelled(&self) {
        self.cancel.cancelled().await
    }
}
