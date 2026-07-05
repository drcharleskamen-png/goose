use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use chrono::{Local, Utc};
use goose_providers::conversation::token_usage::Usage;

use crate::config::Config;
use crate::session::SessionManager;

/// Result of a budget check after a turn's cost has been accumulated.
pub enum BudgetVerdict {
    Ok,
    /// A soft threshold (50/80/95%) was crossed; message for the user.
    Warn(String),
    /// A hard cap was exceeded; the session should stop after this turn.
    Exceeded(String),
}

const WARN_THRESHOLDS: [f64; 3] = [0.5, 0.8, 0.95];

/// Highest warned threshold index per (session, cap-kind), so each soft
/// warning fires once per session run rather than every turn.
static WARNED: LazyLock<Mutex<HashMap<String, usize>>> = LazyLock::new(Default::default);

fn configured_cap(key: &str) -> Option<f64> {
    Config::global()
        .get_param::<f64>(key)
        .ok()
        .filter(|v| *v > 0.0)
}

/// Per-model USD caps from `GOOSE_MAX_MODEL_COST` (a model -> USD map in
/// config.yaml). Entries with non-positive values are ignored.
fn configured_model_caps() -> HashMap<String, f64> {
    Config::global()
        .get_param::<HashMap<String, f64>>("GOOSE_MAX_MODEL_COST")
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, v)| *v > 0.0)
        .collect()
}

/// Lifetime USD cost of one model's accumulated usage, via canonical
/// pricing. Uses the session's provider for lookup, matching /status.
fn model_cost(provider: &str, model: &str, usage: &Usage) -> Option<f64> {
    crate::providers::canonical::maybe_get_canonical_model(provider, model)
        .and_then(|c| c.cost.estimate_cost(usage))
}

fn check_model_cap(spent: f64, cap: f64, model: &str, session_id: &str) -> BudgetVerdict {
    if spent >= cap {
        return BudgetVerdict::Exceeded(format!(
            "🛑 Budget exceeded: model {model} spend ${spent:.4} has reached the ${cap:.2} \
             per-model cap (GOOSE_MAX_MODEL_COST). Session paused — raise the cap or start a \
             new session."
        ));
    }
    if let Some(idx) = crossed_threshold(spent, cap, &format!("{session_id}:model:{model}")) {
        let pct = (WARN_THRESHOLDS[idx] * 100.0) as u32;
        return BudgetVerdict::Warn(format!(
            "⚠️ Budget: model {model} spend ${spent:.4} is over {pct}% of the ${cap:.2} \
             per-model cap."
        ));
    }
    BudgetVerdict::Ok
}

fn crossed_threshold(spent: f64, cap: f64, warned_key: &str) -> Option<usize> {
    let ratio = spent / cap;
    let level = WARN_THRESHOLDS.iter().rposition(|t| ratio >= *t)? + 1;
    let mut warned = WARNED.lock().unwrap();
    let entry = warned.entry(warned_key.to_string()).or_insert(0);
    if level > *entry {
        *entry = level;
        Some(level - 1)
    } else {
        None
    }
}

fn check_cap(spent: f64, cap: f64, kind: &str, session_id: &str) -> BudgetVerdict {
    if spent >= cap {
        return BudgetVerdict::Exceeded(format!(
            "🛑 Budget exceeded: {kind} spend ${spent:.4} has reached the ${cap:.2} cap \
             (GOOSE_MAX_{}_COST). Session paused — raise the cap or start a new session.",
            kind.to_uppercase()
        ));
    }
    if let Some(idx) = crossed_threshold(spent, cap, &format!("{session_id}:{kind}")) {
        let pct = (WARN_THRESHOLDS[idx] * 100.0) as u32;
        return BudgetVerdict::Warn(format!(
            "⚠️ Budget: {kind} spend ${spent:.4} is over {pct}% of the ${cap:.2} cap."
        ));
    }
    BudgetVerdict::Ok
}

/// Check session, daily, and per-model spend against `GOOSE_MAX_SESSION_COST` /
/// `GOOSE_MAX_DAILY_COST` / `GOOSE_MAX_MODEL_COST` (USD). Returns the most
/// severe verdict across all configured caps.
pub async fn check(session_manager: &SessionManager, session_id: &str) -> BudgetVerdict {
    let session_cap = configured_cap("GOOSE_MAX_SESSION_COST");
    let daily_cap = configured_cap("GOOSE_MAX_DAILY_COST");
    let model_caps = configured_model_caps();
    if session_cap.is_none() && daily_cap.is_none() && model_caps.is_empty() {
        return BudgetVerdict::Ok;
    }

    let mut warning = None;

    // Session and per-model caps both read the session row; fetch once.
    if session_cap.is_some() || !model_caps.is_empty() {
        let session = match session_manager.get_session(session_id, false).await {
            Ok(s) => s,
            Err(_) => return BudgetVerdict::Ok,
        };
        if let Some(cap) = session_cap {
            let spent = session.accumulated_cost.unwrap_or(0.0);
            match check_cap(spent, cap, "session", session_id) {
                BudgetVerdict::Ok => {}
                BudgetVerdict::Warn(msg) => warning = Some(msg),
                exceeded => return exceeded,
            }
        }
        if !model_caps.is_empty() {
            if let Some(pm) = session.per_model_usage.as_ref() {
                let provider = session.provider_name.as_deref().unwrap_or("");
                // Deterministic order: sort capped models so the first
                // exceeded one reported is stable.
                let mut capped: Vec<(&String, &Usage)> = pm
                    .iter()
                    .filter(|(m, _)| model_caps.contains_key(*m))
                    .collect();
                capped.sort_by(|a, b| a.0.cmp(b.0));
                for (model, usage) in capped {
                    let cap = model_caps[model];
                    if let Some(spent) = model_cost(provider, model, usage) {
                        match check_model_cap(spent, cap, model, session_id) {
                            BudgetVerdict::Ok => {}
                            BudgetVerdict::Warn(msg) => warning = warning.or(Some(msg)),
                            exceeded => return exceeded,
                        }
                    }
                }
            }
        }
    }

    if let Some(cap) = daily_cap {
        let midnight_local = Local::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .and_then(|dt| dt.and_local_timezone(Local).single())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);
        if let Ok(spent) = session_manager
            .total_accumulated_cost_since(midnight_local)
            .await
        {
            match check_cap(spent, cap, "daily", session_id) {
                BudgetVerdict::Ok => {}
                BudgetVerdict::Warn(msg) => warning = warning.or(Some(msg)),
                exceeded => return exceeded,
            }
        }
    }

    match warning {
        Some(msg) => BudgetVerdict::Warn(msg),
        None => BudgetVerdict::Ok,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exceeded_at_cap() {
        assert!(matches!(
            check_cap(5.0, 5.0, "session", "t1"),
            BudgetVerdict::Exceeded(_)
        ));
    }

    #[test]
    fn warns_once_per_threshold() {
        assert!(matches!(
            check_cap(2.6, 5.0, "session", "t2"),
            BudgetVerdict::Warn(_)
        ));
        assert!(matches!(
            check_cap(2.7, 5.0, "session", "t2"),
            BudgetVerdict::Ok
        ));
        assert!(matches!(
            check_cap(4.1, 5.0, "session", "t2"),
            BudgetVerdict::Warn(_)
        ));
    }

    #[test]
    fn under_first_threshold_is_ok() {
        assert!(matches!(
            check_cap(1.0, 5.0, "session", "t3"),
            BudgetVerdict::Ok
        ));
    }

    #[test]
    fn model_cap_exceeded_at_cap() {
        assert!(matches!(
            check_model_cap(5.0, 5.0, "glm-5.2", "m1"),
            BudgetVerdict::Exceeded(_)
        ));
    }

    #[test]
    fn model_cap_warns_once_per_threshold() {
        assert!(matches!(
            check_model_cap(2.6, 5.0, "glm-5.2", "m2"),
            BudgetVerdict::Warn(_)
        ));
        // Same threshold band -> no re-warn.
        assert!(matches!(
            check_model_cap(2.7, 5.0, "glm-5.2", "m2"),
            BudgetVerdict::Ok
        ));
        // Distinct model -> independent warning state.
        assert!(matches!(
            check_model_cap(2.6, 5.0, "deepseek-chat", "m2"),
            BudgetVerdict::Warn(_)
        ));
        // Higher threshold on original model -> warns again.
        assert!(matches!(
            check_model_cap(4.1, 5.0, "glm-5.2", "m2"),
            BudgetVerdict::Warn(_)
        ));
    }

    #[test]
    fn model_cap_under_threshold_is_ok() {
        assert!(matches!(
            check_model_cap(1.0, 5.0, "glm-5.2", "m3"),
            BudgetVerdict::Ok
        ));
    }
}
