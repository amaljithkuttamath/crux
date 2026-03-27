/// API-equivalent cost estimates per million tokens
/// Based on Anthropic's published pricing (updated March 2026)
/// Opus/Haiku rates are for 4.5/4.6 generation. Sonnet unchanged.
/// Cache write = 5-minute TTL (1.25x input). Cache read = 0.1x input.
pub struct ModelPricing {
    pub input_per_m: f64,
    pub output_per_m: f64,
    pub cache_write_per_m: f64,
    pub cache_read_per_m: f64,
}

pub fn pricing_for_model(model: &str) -> ModelPricing {
    // Claude models
    if model.contains("opus") {
        ModelPricing {
            input_per_m: 5.0,
            output_per_m: 25.0,
            cache_write_per_m: 6.25,
            cache_read_per_m: 0.50,
        }
    } else if model.contains("haiku") {
        ModelPricing {
            input_per_m: 1.0,
            output_per_m: 5.0,
            cache_write_per_m: 1.25,
            cache_read_per_m: 0.10,
        }
    } else if model.contains("sonnet") {
        ModelPricing {
            input_per_m: 3.0,
            output_per_m: 15.0,
            cache_write_per_m: 3.75,
            cache_read_per_m: 0.30,
        }
    // Non-Claude models (Cursor) - approximate public API rates, no cache semantics
    } else if model.contains("gpt-5") || model.contains("codex") {
        ModelPricing {
            input_per_m: 2.50,
            output_per_m: 10.0,
            cache_write_per_m: 0.0,
            cache_read_per_m: 0.0,
        }
    } else if model.contains("grok") {
        ModelPricing {
            input_per_m: 3.0,
            output_per_m: 15.0,
            cache_write_per_m: 0.0,
            cache_read_per_m: 0.0,
        }
    } else if model.contains("gemini") {
        ModelPricing {
            input_per_m: 1.25,
            output_per_m: 10.0,
            cache_write_per_m: 0.0,
            cache_read_per_m: 0.0,
        }
    } else if model.contains("supernova") {
        ModelPricing {
            input_per_m: 3.0,
            output_per_m: 15.0,
            cache_write_per_m: 0.0,
            cache_read_per_m: 0.0,
        }
    } else if model.contains("deepseek") {
        ModelPricing {
            input_per_m: 0.27,
            output_per_m: 1.10,
            cache_write_per_m: 0.0,
            cache_read_per_m: 0.0,
        }
    } else {
        // Default fallback (sonnet-equivalent)
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
