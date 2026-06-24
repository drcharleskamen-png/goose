//! Complexity-ordered model ladder.
//!
//! A ladder is an ordered list of model names, cheapest first, paired with
//! ascending band thresholds that map a complexity score to a rung. For a
//! 3-model ladder `[low, mid, top]` with bands `[0.40, 0.65]`:
//!
//! ```text
//! complexity < 0.40        -> low
//! 0.40 <= complexity < 0.65 -> mid
//! complexity >= 0.65        -> top
//! ```
//!
//! The model names come from `GOOSE_ROUTER_LADDER` (comma-separated, cheap →
//! dear). The bands are a property of the trained bundle (its `config.json`
//! carries fitted defaults); `GOOSE_ROUTER_BANDS` overrides them. When neither
//! supplies usable bands we fall back to evenly spaced thresholds so the ladder
//! still functions.

/// An ordered ladder of models with the bands that select between them.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelLadder {
    models: Vec<String>,
    bands: Vec<f32>,
}

impl ModelLadder {
    /// Build a ladder from `GOOSE_ROUTER_LADDER` (model names) and bands.
    ///
    /// Band resolution order: `GOOSE_ROUTER_BANDS` env override → the bundle's
    /// `default_bands` → evenly spaced thresholds. Returns `None` when no
    /// ladder is configured (cost savings routing then stays on the main model).
    pub fn from_config(default_bands: Option<Vec<f32>>) -> Option<Self> {
        let spec = std::env::var("GOOSE_ROUTER_LADDER").ok()?;
        let models: Vec<String> = spec
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if models.is_empty() {
            return None;
        }

        let bands = bands_from_env()
            .or(default_bands)
            .filter(|b| is_ascending(b))
            .unwrap_or_else(|| evenly_spaced_bands(models.len()));

        Some(Self { models, bands })
    }

    /// Construct directly (used by tests and the bundle loader).
    pub fn new(models: Vec<String>, bands: Vec<f32>) -> Self {
        Self { models, bands }
    }

    /// Select the model for `complexity`.
    ///
    /// The rung index is the number of bands the score meets or exceeds,
    /// clamped to the ladder length. When the band count doesn't match
    /// `models.len() - 1`, the index is scaled proportionally so a ladder still
    /// works with mismatched bands.
    pub fn select(&self, complexity: f32) -> &str {
        debug_assert!(!self.models.is_empty());
        let crossed = self.bands.iter().filter(|&&b| complexity >= b).count();

        let idx = if self.bands.len() + 1 == self.models.len() {
            crossed
        } else if self.bands.is_empty() {
            0
        } else {
            let frac = crossed as f32 / (self.bands.len() + 1) as f32;
            (frac * self.models.len() as f32) as usize
        };

        let idx = idx.min(self.models.len() - 1);
        &self.models[idx]
    }

    pub fn models(&self) -> &[String] {
        &self.models
    }

    pub fn bands(&self) -> &[f32] {
        &self.bands
    }
}

fn bands_from_env() -> Option<Vec<f32>> {
    let raw = std::env::var("GOOSE_ROUTER_BANDS").ok()?;
    let bands: Vec<f32> = raw
        .split(',')
        .filter_map(|s| s.trim().parse::<f32>().ok())
        .filter(|v| (0.0..=1.0).contains(v))
        .collect();
    if bands.is_empty() {
        tracing::warn!(
            target: "goose::router",
            "ignoring invalid GOOSE_ROUTER_BANDS {:?}",
            raw
        );
        return None;
    }
    if !is_ascending(&bands) {
        tracing::warn!(
            target: "goose::router",
            "ignoring non-ascending GOOSE_ROUTER_BANDS {:?}",
            bands
        );
        return None;
    }
    Some(bands)
}

fn is_ascending(bands: &[f32]) -> bool {
    bands.windows(2).all(|w| w[0] <= w[1])
}

fn evenly_spaced_bands(model_count: usize) -> Vec<f32> {
    if model_count < 2 {
        return Vec::new();
    }
    (1..model_count)
        .map(|i| i as f32 / model_count as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ladder(models: &[&str], bands: &[f32]) -> ModelLadder {
        ModelLadder::new(
            models.iter().map(|s| s.to_string()).collect(),
            bands.to_vec(),
        )
    }

    #[test]
    fn three_tier_band_selection() {
        let l = ladder(&["low", "mid", "top"], &[0.40, 0.65]);
        assert_eq!(l.select(0.0), "low");
        assert_eq!(l.select(0.39), "low");
        assert_eq!(l.select(0.40), "mid");
        assert_eq!(l.select(0.64), "mid");
        assert_eq!(l.select(0.65), "top");
        assert_eq!(l.select(1.0), "top");
    }

    #[test]
    fn single_band_two_models() {
        let l = ladder(&["fast", "main"], &[0.30]);
        assert_eq!(l.select(0.29), "fast");
        assert_eq!(l.select(0.30), "main");
    }

    #[test]
    fn empty_bands_always_cheapest() {
        let l = ladder(&["fast", "main"], &[]);
        assert_eq!(l.select(0.0), "fast");
        assert_eq!(l.select(1.0), "fast");
    }

    #[test]
    fn mismatched_bands_scale_proportionally() {
        // 3 models but only 1 band: proportional fallback, never panics.
        let l = ladder(&["a", "b", "c"], &[0.5]);
        let _ = l.select(0.0);
        let top = l.select(1.0);
        assert!(["a", "b", "c"].contains(&top));
    }

    #[test]
    fn evenly_spaced_default() {
        assert_eq!(evenly_spaced_bands(1), Vec::<f32>::new());
        assert_eq!(evenly_spaced_bands(2), vec![0.5]);
        let three = evenly_spaced_bands(3);
        assert!((three[0] - 1.0 / 3.0).abs() < 1e-6);
        assert!((three[1] - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn ascending_check() {
        assert!(is_ascending(&[0.1, 0.4, 0.65]));
        assert!(!is_ascending(&[0.65, 0.4]));
    }
}
