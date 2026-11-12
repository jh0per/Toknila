//! The sums a person looks at: totals, a per-day series and breakdowns.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{PriceTable, TokenCounts, day_label, day_of};
use crate::Timestamp;

/// One model call with what it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub at: Timestamp,
    pub model: String,
    pub tokens: TokenCounts,
    /// Key of the card whose run made the call, when a Toknila run did.
    pub card: Option<String>,
    pub chain: Option<String>,
    pub project: Option<String>,
    /// The folder the session worked in.
    pub workspace: String,
}

/// The numbers of one group of calls.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageFigures {
    pub tokens: TokenCounts,
    /// Every token processed: input, cache and output.
    pub processed: u64,
    /// Input tokens not served from the cache.
    pub uncached_input: u64,
    /// The share of input tokens served from the cache, 0 to 1.
    pub cached_share: f64,
    /// Estimated cost at API list prices, USD.
    pub cost_usd: f64,
    /// What the cache saved at those prices, USD.
    pub cache_savings_usd: f64,
    /// Model calls counted.
    pub messages: u64,
}

/// One line of a breakdown.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageItem {
    pub label: String,
    pub figures: UsageFigures,
    /// A workspace line only: the folder of the main repository the folder belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// A workspace line only: the last folder name of `repo`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// One UTC day of the series.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DayUsage {
    /// `YYYY-MM-DD`, UTC.
    pub day: String,
    pub figures: UsageFigures,
}

/// Usage over the last `days` days.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageSummary {
    pub days: u32,
    pub totals: UsageFigures,
    /// One entry per day, oldest first, empty days included.
    pub series: Vec<DayUsage>,
    pub by_model: Vec<UsageItem>,
    /// Toknila runs only, as are the next two.
    pub by_project: Vec<UsageItem>,
    pub by_chain: Vec<UsageItem>,
    pub by_card: Vec<UsageItem>,
    pub by_workspace: Vec<UsageItem>,
    /// Models seen without a price: counted in tokens, not in cost.
    pub unpriced_models: Vec<String>,
}

/// The figures of `rows`, costed with `prices`.
pub fn figures<'a>(rows: impl IntoIterator<Item = &'a Row>, prices: &PriceTable) -> UsageFigures {
    let mut f = UsageFigures::default();
    for row in rows {
        f.tokens.add(&row.tokens);
        f.messages += 1;
        if let Some(price) = prices.lookup(&row.model) {
            f.cost_usd += price.cost(&row.tokens);
            f.cache_savings_usd += price.cache_savings(&row.tokens);
        }
    }
    f.processed = f.tokens.processed();
    f.uncached_input = f.tokens.uncached_input();
    let input = f.uncached_input + f.tokens.cache_read;
    f.cached_share = if input == 0 {
        0.0
    } else {
        f.tokens.cache_read as f64 / input as f64
    };
    f
}

/// The figures of each card (by ticket key) that made calls in `rows`.
pub fn figures_per_card(rows: &[Row], prices: &PriceTable) -> BTreeMap<String, UsageFigures> {
    let mut by_card: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
    for row in rows {
        if let Some(card) = &row.card {
            by_card.entry(card).or_default().push(row);
        }
    }
    by_card
        .into_iter()
        .map(|(card, rows)| (card.to_owned(), figures(rows, prices)))
        .collect()
}

/// Groups `rows` by the label `by` gives (rows without one are left out) and orders the groups
/// by cost, then tokens, then label.
fn breakdown<'a>(
    rows: &[&'a Row],
    prices: &PriceTable,
    by: impl Fn(&Row) -> Option<String>,
) -> Vec<UsageItem> {
    let mut groups: BTreeMap<String, Vec<&'a Row>> = BTreeMap::new();
    for row in rows {
        if let Some(label) = by(row) {
            groups.entry(label).or_default().push(row);
        }
    }
    let mut items: Vec<UsageItem> = groups
        .into_iter()
        .map(|(label, rows)| UsageItem {
            label,
            figures: figures(rows, prices),
            repo: None,
            name: None,
        })
        .collect();
    items.sort_by(|a, b| {
        b.figures
            .cost_usd
            .total_cmp(&a.figures.cost_usd)
            .then(b.figures.processed.cmp(&a.figures.processed))
            .then(a.label.cmp(&b.label))
    });
    items
}

/// Summarises the last `days` UTC days up to `now`; rows outside them are ignored.
pub fn summarize(rows: &[Row], prices: &PriceTable, now: Timestamp, days: u32) -> UsageSummary {
    let last = day_of(now);
    let first = last - i64::from(days.max(1)) + 1;
    let inside: Vec<&Row> = rows
        .iter()
        .filter(|r| (first..=last).contains(&day_of(r.at)))
        .collect();
    let series = (first..=last)
        .map(|day| DayUsage {
            day: day_label(day),
            figures: figures(
                inside.iter().copied().filter(|r| day_of(r.at) == day),
                prices,
            ),
        })
        .collect();
    let unpriced: BTreeSet<&str> = inside
        .iter()
        .filter(|r| prices.lookup(&r.model).is_none())
        .map(|r| r.model.as_str())
        .collect();
    UsageSummary {
        days,
        totals: figures(inside.iter().copied(), prices),
        series,
        by_model: breakdown(&inside, prices, |r| Some(r.model.clone())),
        by_project: breakdown(&inside, prices, |r| r.project.clone()),
        by_chain: breakdown(&inside, prices, |r| r.chain.clone()),
        by_card: breakdown(&inside, prices, |r| r.card.clone()),
        by_workspace: breakdown(&inside, prices, |r| Some(r.workspace.clone())),
        unpriced_models: unpriced.into_iter().map(str::to_owned).collect(),
    }
}
