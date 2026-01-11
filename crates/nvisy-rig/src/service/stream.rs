//! Streaming utilities and usage statistics.

use serde::{Deserialize, Serialize};

/// Token usage statistics for a chat completion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageStats {
    /// Number of input tokens (prompt).
    pub input_tokens: u32,

    /// Number of output tokens (completion).
    pub output_tokens: u32,

    /// Number of tokens used for reasoning/thinking.
    pub reasoning_tokens: u32,

    /// Total tokens (input + output).
    pub total_tokens: u32,

    /// Estimated cost in USD (if available).
    pub estimated_cost_usd: Option<f64>,
}

impl UsageStats {
    /// Creates new usage stats.
    pub fn new(input_tokens: u32, output_tokens: u32) -> Self {
        Self {
            input_tokens,
            output_tokens,
            reasoning_tokens: 0,
            total_tokens: input_tokens + output_tokens,
            estimated_cost_usd: None,
        }
    }

    /// Adds reasoning tokens.
    pub fn with_reasoning_tokens(mut self, reasoning_tokens: u32) -> Self {
        self.reasoning_tokens = reasoning_tokens;
        self
    }

    /// Sets the estimated cost.
    pub fn with_cost(mut self, cost_usd: f64) -> Self {
        self.estimated_cost_usd = Some(cost_usd);
        self
    }

    /// Accumulates usage from another stats instance.
    pub fn accumulate(&mut self, other: &UsageStats) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.reasoning_tokens += other.reasoning_tokens;
        self.total_tokens += other.total_tokens;

        if let Some(other_cost) = other.estimated_cost_usd {
            self.estimated_cost_usd = Some(self.estimated_cost_usd.unwrap_or(0.0) + other_cost);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_stats_new() {
        let stats = UsageStats::new(100, 50);
        assert_eq!(stats.input_tokens, 100);
        assert_eq!(stats.output_tokens, 50);
        assert_eq!(stats.total_tokens, 150);
    }

    #[test]
    fn usage_stats_accumulate() {
        let mut stats = UsageStats::new(100, 50);
        let other = UsageStats::new(200, 100).with_cost(0.01);

        stats.accumulate(&other);

        assert_eq!(stats.input_tokens, 300);
        assert_eq!(stats.output_tokens, 150);
        // 150 (original) + 300 (other) = 450
        assert_eq!(stats.total_tokens, 450);
        assert_eq!(stats.estimated_cost_usd, Some(0.01));
    }
}
