//! The price table: USD per million tokens, per model.

use serde::Deserialize;

use super::TokenCounts;

/// Prices are per this many tokens.
const PER: f64 = 1_000_000.0;

const DEFAULTS: &str = include_str!("prices.toml");

/// What went wrong reading a price file.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PriceError {
    #[error("the price file is not valid: {0}")]
    Invalid(String),
    #[error("model {0}: a price cannot be negative")]
    Negative(String),
}

/// What one model costs, in USD per million tokens.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ModelPrice {
    /// The model id as transcripts name it, without a date suffix.
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub input: f64,
    /// Writing to the cache (the 5-minute rate).
    pub cache_write: f64,
    pub cache_read: f64,
    pub output: f64,
}

impl ModelPrice {
    /// The estimated cost of `t` in USD.
    pub fn cost(&self, t: &TokenCounts) -> f64 {
        (t.input as f64 * self.input
            + t.cache_write as f64 * self.cache_write
            + t.cache_read as f64 * self.cache_read
            + t.output as f64 * self.output)
            / PER
    }

    /// What the cache saved on `t`: its cached reads priced as plain input, minus what they cost.
    pub fn cache_savings(&self, t: &TokenCounts) -> f64 {
        t.cache_read as f64 * (self.input - self.cache_read) / PER
    }
}

/// The prices of all models, and where they came from.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PriceTable {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub retrieved: String,
    #[serde(default, rename = "model")]
    pub models: Vec<ModelPrice>,
}

impl PriceTable {
    /// The prices shipped with Toknila.
    pub fn defaults() -> PriceTable {
        PriceTable::parse(DEFAULTS).expect("the shipped prices.toml is valid (checked by a test)")
    }

    /// Reads a price file.
    pub fn parse(text: &str) -> Result<PriceTable, PriceError> {
        let table: PriceTable =
            toml::from_str(text).map_err(|e| PriceError::Invalid(e.message().to_owned()))?;
        for m in &table.models {
            let prices = [m.input, m.cache_write, m.cache_read, m.output];
            if prices.iter().any(|p| *p < 0.0 || !p.is_finite()) {
                return Err(PriceError::Negative(m.id.clone()));
            }
        }
        Ok(table)
    }

    /// This table with `over`'s models replacing or adding to its own.
    pub fn with_overrides(mut self, over: PriceTable) -> PriceTable {
        for m in over.models {
            match self.models.iter_mut().find(|own| own.id == m.id) {
                Some(own) => *own = m,
                None => self.models.push(m),
            }
        }
        self
    }

    /// The price of `model`, matched by id. A date suffix (`-20251001`) or a context marker
    /// (`[1m]`) after the id is ignored; any other longer name is another model.
    pub fn lookup(&self, model: &str) -> Option<&ModelPrice> {
        self.models.iter().find(|p| {
            model.strip_prefix(p.id.as_str()).is_some_and(|rest| {
                rest.is_empty()
                    || rest.starts_with('[')
                    || rest.strip_prefix('-').is_some_and(|d| {
                        d.get(..8)
                            .is_some_and(|y| y.bytes().all(|b| b.is_ascii_digit()))
                    })
            })
        })
    }
}
