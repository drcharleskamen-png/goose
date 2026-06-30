//! Native cost-saving model router for goose.
//!
//! A [`Router`] scores the current conversation and decides which model a turn
//! should be served by. This is the engine behind goose's *cost savings mode*:
//! simple turns go to a small/cheap model, hard turns stay on the frontier
//! model.
//!
//! The only strategy is [`EmbeddingRouter`]: it renders the conversation,
//! embeds it with a small ONNX encoder, and runs an MLP head to produce a
//! complexity score in `[0, 1]`. The score is mapped to a model on a
//! complexity-ordered *ladder* using band thresholds that ship with the trained
//! bundle. Everything runs in-process, fully offline, from a local bundle —
//! there is no sidecar or network dependency.

mod embedding;
mod ladder;
mod render;

pub use embedding::EmbeddingRouter;
pub use ladder::ModelLadder;
pub use render::render_for_routing;

use goose_providers::conversation::Conversation;
use std::path::PathBuf;

/// The default on-disk location for the complexity model bundle
/// (`~/.goose/complexity_model/`), or `None` if the home directory can't be
/// resolved.
pub fn default_bundle_dir() -> Option<PathBuf> {
    embedding::default_bundle_dir()
}

/// Whether a usable bundle is already installed at the default location.
pub fn bundle_present() -> bool {
    embedding::bundle_present()
}

/// What the agent loop needs to know after routing a turn.
#[derive(Debug, Clone)]
pub struct RouteDecision {
    /// Estimated cognitive complexity of the turn in `[0, 1]`.
    pub complexity: f32,
    /// The ladder model name selected for this turn, or `None` to keep the
    /// main model.
    pub selected_model: Option<String>,
    /// How long the routing decision took.
    pub elapsed_ms: u64,
}

/// A pluggable strategy that scores a turn and selects a model.
///
/// Implementations are expected to be cheap to clone (wrap heavy state in an
/// `Arc`) and safe to call from the async agent loop on a blocking section.
pub trait Router: Send + Sync {
    /// Human-readable name of the strategy, for logs and diagnostics.
    fn name(&self) -> &'static str;

    /// Score `conversation` and decide which model to use.
    ///
    /// Returns `None` when no decision can be made (e.g. the conversation has
    /// no user turn). Callers should treat `None` as "use the main model".
    fn route(&self, conversation: &Conversation) -> Option<RouteDecision>;
}

/// Load the cost-savings router from the default bundle location.
///
/// Returns `None` (not an error) when no bundle is available — callers should
/// treat that as "cost savings routing disabled".
pub fn load_default_router() -> Option<Box<dyn Router>> {
    EmbeddingRouter::try_load_default().map(|r| Box::new(r) as Box<dyn Router>)
}
