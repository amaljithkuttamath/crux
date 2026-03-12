/// API-equivalent cost estimates per million tokens
/// Based on Anthropic's published pricing (as of March 2026)
/// These are what it WOULD cost on the API, not what the user pays on subscription
pub struct ModelPricing {
    pub input_per_m: f64,
    pub output_per_m: f64,
    pub cache_write_per_m: f64,
    pub cache_read_per_m: f64,
}

pub fn pricing_for_model(model: &str) -> ModelPricing {
    if model.contains("opus") {
        ModelPricing {
            input_per_m: 15.0,
            output_per_m: 75.0,
            cache_write_per_m: 18.75,
            cache_read_per_m: 1.50,
        }
    } else if model.contains("haiku") {
        ModelPricing {
            input_per_m: 0.80,
            output_per_m: 4.0,
            cache_write_per_m: 1.0,
            cache_read_per_m: 0.08,
        }
    } else {
        // sonnet (default)
        ModelPricing {
            input_per_m: 3.0,
            output_per_m: 15.0,
            cache_write_per_m: 3.75,
            cache_read_per_m: 0.30,
        }
    }
}


pub fn estimate_cost(
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
) -> f64 {
    let p = pricing_for_model(model);
    (input_tokens as f64 / 1_000_000.0 * p.input_per_m)
        + (output_tokens as f64 / 1_000_000.0 * p.output_per_m)
        + (cache_creation_tokens as f64 / 1_000_000.0 * p.cache_write_per_m)
        + (cache_read_tokens as f64 / 1_000_000.0 * p.cache_read_per_m)
}

pub fn format_cost(cost: f64) -> String {
    if cost >= 0.01 {
        format!("${:.2}", cost)
    } else {
        format!("${:.3}", cost)
    }
}
