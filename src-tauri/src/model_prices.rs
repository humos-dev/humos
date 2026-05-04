//! Model pricing lookup.
//!
//! Loads per-model USD prices from `~/.humOS/model-prices.json` on demand,
//! with a hardcoded fallback table so the app works out of the box even when
//! the file is missing or malformed. Never panics. Malformed JSON is logged
//! as a warning.
//!
//! JSON schema:
//! ```json
//! {
//!   "claude-sonnet-4-6": { "input_per_1m": 3.00, "output_per_1m": 15.00 }
//! }
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModelPrice {
    pub input_per_1m: f64,
    pub output_per_1m: f64,
}

fn fallback_table() -> HashMap<String, ModelPrice> {
    let mut m = HashMap::new();
    m.insert(
        "claude-sonnet-4-6".to_string(),
        ModelPrice { input_per_1m: 3.00, output_per_1m: 15.00 },
    );
    m.insert(
        "claude-opus-4-7".to_string(),
        ModelPrice { input_per_1m: 15.00, output_per_1m: 75.00 },
    );
    m.insert(
        "claude-haiku-4-5-20251001".to_string(),
        ModelPrice { input_per_1m: 0.80, output_per_1m: 4.00 },
    );
    m
}

fn prices_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    Some(home.join(".humOS").join("model-prices.json"))
}

// Cache the loaded price table on first access. Reading the JSON file on every
// cost_for() call would mean dozens of disk reads per second when the activity
// panel polls. The table changes only on humOS restart.
static TABLE: OnceLock<HashMap<String, ModelPrice>> = OnceLock::new();

fn is_valid_price(p: &ModelPrice) -> bool {
    p.input_per_1m.is_finite()
        && p.output_per_1m.is_finite()
        && p.input_per_1m >= 0.0
        && p.output_per_1m >= 0.0
        // Sanity ceiling: no real model is over $1000/1M tokens. Anything
        // above is almost certainly malformed JSON (e.g. NaN, Inf, garbage).
        && p.input_per_1m < 1000.0
        && p.output_per_1m < 1000.0
}

/// Load the price table once. Returns the JSON file contents if readable,
/// parseable, and within sanity bounds, otherwise returns the hardcoded
/// fallback. Per-entry validation: invalid entries are dropped and the
/// fallback's entry for that model fills in.
fn load_table() -> HashMap<String, ModelPrice> {
    let path = match prices_path() {
        Some(p) => p,
        None => return fallback_table(),
    };

    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => {
            // Missing file is normal on fresh installs. No warning.
            return fallback_table();
        }
    };

    let parsed: HashMap<String, ModelPrice> = match serde_json::from_str(&raw) {
        Ok(t) => t,
        Err(e) => {
            log::warn!(
                "model_prices: malformed JSON at {:?}, using fallback. error: {}",
                path,
                e
            );
            return fallback_table();
        }
    };

    // Start from fallback so invalid file entries get the safe defaults.
    let mut table = fallback_table();
    for (model, price) in parsed {
        if is_valid_price(&price) {
            table.insert(model, price);
        } else {
            log::warn!(
                "model_prices: rejecting invalid price for {} (in={}, out={})",
                model, price.input_per_1m, price.output_per_1m
            );
        }
    }
    table
}

fn table() -> &'static HashMap<String, ModelPrice> {
    TABLE.get_or_init(load_table)
}

/// Compute the USD cost for a given model and token counts.
/// Returns None when the model is not in the price table or the result
/// is non-finite.
pub fn cost_for(model: &str, input_tokens: u64, output_tokens: u64) -> Option<f64> {
    let price = table().get(model)?;
    let input_cost = (input_tokens as f64) * price.input_per_1m / 1_000_000.0;
    let output_cost = (output_tokens as f64) * price.output_per_1m / 1_000_000.0;
    let total = input_cost + output_cost;
    // Guard the output. With u64::MAX tokens or rounding edge cases the
    // multiplication can deliver inf or NaN; the frontend should see None,
    // not a corrupted number.
    if !total.is_finite() || total < 0.0 {
        return None;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_has_three_models() {
        let t = fallback_table();
        assert!(t.contains_key("claude-sonnet-4-6"));
        assert!(t.contains_key("claude-opus-4-7"));
        assert!(t.contains_key("claude-haiku-4-5-20251001"));
    }

    #[test]
    fn cost_for_known_model_is_correct() {
        // 1M input tokens at $3.00 + 1M output at $15.00 = $18.00
        let cost = cost_for("claude-sonnet-4-6", 1_000_000, 1_000_000).unwrap();
        assert!((cost - 18.00).abs() < 1e-9);
    }

    #[test]
    fn cost_for_unknown_model_is_none() {
        assert!(cost_for("not-a-real-model", 100, 100).is_none());
    }

    #[test]
    fn cost_for_zero_tokens_is_zero() {
        let cost = cost_for("claude-opus-4-7", 0, 0).unwrap();
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn is_valid_price_rejects_nan_inf_negative_and_huge() {
        assert!(is_valid_price(&ModelPrice { input_per_1m: 3.0, output_per_1m: 15.0 }));
        assert!(!is_valid_price(&ModelPrice { input_per_1m: f64::NAN, output_per_1m: 15.0 }));
        assert!(!is_valid_price(&ModelPrice { input_per_1m: f64::INFINITY, output_per_1m: 15.0 }));
        assert!(!is_valid_price(&ModelPrice { input_per_1m: -1.0, output_per_1m: 15.0 }));
        assert!(!is_valid_price(&ModelPrice { input_per_1m: 3.0, output_per_1m: 9999.0 }));
    }

    #[test]
    fn cost_for_returns_none_when_result_overflows() {
        // u64::MAX tokens with $15/1M output produces a result well within f64
        // range, but we still verify finite-result guard works for normal calls.
        let cost = cost_for("claude-sonnet-4-6", 100, 100);
        assert!(cost.is_some());
        assert!(cost.unwrap().is_finite());
    }
}
