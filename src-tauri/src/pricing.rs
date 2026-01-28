pub fn cost_usd(
    prompt_tokens: i64,
    completion_tokens: i64,
    prompt_price_per_1k: f64,
    completion_price_per_1k: f64,
) -> f64 {
    (prompt_tokens as f64 / 1000.0) * prompt_price_per_1k
        + (completion_tokens as f64 / 1000.0) * completion_price_per_1k
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calc_cost() {
        assert!((cost_usd(1000, 2000, 1.0, 2.0) - 5.0).abs() < 1e-6)
    }
}
