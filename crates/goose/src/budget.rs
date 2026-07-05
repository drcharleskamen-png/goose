use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use chrono::{Local, Utc};

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

/// Check session and daily spend against `GOOSE_MAX_SESSION_COST` /
/// `GOOSE_MAX_DAILY_COST` (USD). Returns the most severe verdict.
pub async fn check(session_manager: &SessionManager, session_id: &str) -> BudgetVerdict {
    let session_cap = configured_cap("GOOSE_MAX_SESSION_COST");
    let daily_cap = configured_cap("GOOSE_MAX_DAILY_COST");
    if session_cap.is_none() && daily_cap.is_none() {
        return BudgetVerdict::Ok;
    }

    let mut warning = None;

    if let Some(cap) = session_cap {
        let spent = match session_manager.get_session(session_id, false).await {
            Ok(session) => session.accumulated_cost.unwrap_or(0.0),
            Err(_) => return BudgetVerdict::Ok,
        };
        match check_cap(spent, cap, "session", session_id) {
            BudgetVerdict::Ok => {}
            BudgetVerdict::Warn(msg) => warning = Some(msg),
            exceeded => return exceeded,
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
}
