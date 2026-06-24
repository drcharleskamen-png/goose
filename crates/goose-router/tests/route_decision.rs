//! End-to-end decision test for the embedding router on real goose
//! `Conversation` objects, using the installed bundle at
//! `~/.goose/complexity_model/`.
//!
//! Proves the full `Router::route` path (render → score → ladder) yields
//! sensible cost-saving decisions: a trivial turn selects a cheaper ladder
//! model, a demanding multi-step turn selects a dearer one.
//!
//! Skipped (not failed) if the bundle is absent.

use std::path::PathBuf;

use goose_providers::conversation::{message::Message, Conversation};
use goose_router::{EmbeddingRouter, Router};

fn bundle_dir() -> PathBuf {
    dirs::home_dir()
        .expect("home dir")
        .join(".goose")
        .join("complexity_model")
}

#[test]
fn simple_turn_routes_cheaper_than_complex() {
    let dir = bundle_dir();
    if !dir.join("config.json").exists() {
        eprintln!("skipping: no bundle at {}", dir.display());
        return;
    }

    // Pin a 3-tier ladder for the duration of this test so routing produces a
    // model selection regardless of the host environment.
    std::env::set_var("GOOSE_ROUTER_LADDER", "fast,mid,frontier");
    std::env::set_var("GOOSE_ROUTER_BANDS", "0.40,0.65");

    let router = EmbeddingRouter::load_from_dir(&dir).expect("load bundle");

    let simple = Conversation::new_unvalidated(vec![Message::user().with_text("hi, how are you?")]);
    let complex = Conversation::new_unvalidated(vec![Message::user().with_text(
        "Refactor this multi-threaded Rust web server to use a bounded work-stealing \
         scheduler, prove the absence of data races, add graceful shutdown with \
         in-flight request draining, instrument it with OpenTelemetry spans, and write \
         property-based tests covering backpressure under load.",
    )]);

    let simple_decision = router.route(&simple).expect("route simple");
    let complex_decision = router.route(&complex).expect("route complex");

    eprintln!(
        "simple: complexity={:.4} model={:?} ({}ms)",
        simple_decision.complexity, simple_decision.selected_model, simple_decision.elapsed_ms
    );
    eprintln!(
        "complex: complexity={:.4} model={:?} ({}ms)",
        complex_decision.complexity, complex_decision.selected_model, complex_decision.elapsed_ms
    );

    std::env::remove_var("GOOSE_ROUTER_LADDER");
    std::env::remove_var("GOOSE_ROUTER_BANDS");

    assert!(
        simple_decision.complexity < complex_decision.complexity,
        "simple turn should score lower complexity than complex turn",
    );
    assert_eq!(
        simple_decision.selected_model.as_deref(),
        Some("fast"),
        "trivial greeting should route to the cheapest rung (complexity {:.4})",
        simple_decision.complexity,
    );
    assert_ne!(
        complex_decision.selected_model.as_deref(),
        Some("fast"),
        "demanding engineering task should not route to the cheapest rung (complexity {:.4})",
        complex_decision.complexity,
    );
}
