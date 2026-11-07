//! Token usage and its estimated cost: reading agent transcripts, a price table, and the sums a
//! person looks at. Everything here is plain data in and out; the daemon reads the files.
//!
//! Costs are estimates at API list prices. On a subscription the real cost differs.

mod civil;
mod price;
pub mod quota;
mod summary;
mod transcript;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use crate::Timestamp;

pub use civil::{day_label, day_of, parse_rfc3339};
pub use price::{LongPromptPrice, ModelPrice, PriceError, PriceTable};
pub use summary::{
    DayUsage, Row, UsageFigures, UsageItem, UsageSummary, figures, figures_per_card, summarize,
};
pub use transcript::{Chunk, ClaudeCode, TranscriptFormat, accepted_transcript, session_of};

/// The windows, in days, the usage page can show; the first is the default.
pub const WINDOWS: [u32; 3] = [7, 30, 90];

/// The four token counts a model call reports.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenCounts {
    /// Input tokens that were neither read from nor written to the cache.
    pub input: u64,
    /// Input tokens written to the cache.
    pub cache_write: u64,
    /// Input tokens served from the cache.
    pub cache_read: u64,
    pub output: u64,
}

impl TokenCounts {
    /// Every token the model processed.
    pub fn processed(&self) -> u64 {
        self.input + self.cache_write + self.cache_read + self.output
    }

    /// Input tokens that were not served from the cache.
    pub fn uncached_input(&self) -> u64 {
        self.input + self.cache_write
    }

    /// Adds `other` to `self`.
    pub fn add(&mut self, other: &TokenCounts) {
        self.input += other.input;
        self.cache_write += other.cache_write;
        self.cache_read += other.cache_read;
        self.output += other.output;
    }
}

/// One model call found in a transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Identifies the call across the repeated lines a transcript writes for it.
    pub key: String,
    pub at: Timestamp,
    pub model: String,
    pub tokens: TokenCounts,
}
