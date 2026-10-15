//! *Tokens over time*: one stacked bar per hour or day, a part per account (and, for the old app, per CLI).

use std::collections::HashSet;

use super::{
  AccountUsage, CliSeries, Grouping,
  period::{Bucket, DayRange, bucket_of},
  report::Within,
};
use crate::{id::RunId, run::TokenUsage};

/// What the bars measure (*Spent* / *Cached* / *Output*).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Measure {
  #[default]
  Spent,
  Cached,
  Output,
}

impl Measure {
  pub const fn of(self, usage: TokenUsage) -> u64 {
    match self {
      Self::Spent => usage.counted(),
      Self::Cached => usage.cached_input,
      Self::Output => usage.output,
    }
  }
}

/// One bar: its hour or day, each account's usage in it, each CLI's (indexed by [`CliSeries::index`]) and
/// the runs whose turns completed in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeriesBar {
  pub bucket: Bucket,
  pub by_cli: [TokenUsage; 3],
  pub by_account: AccountUsage,
  pub runs: usize,
}

impl SeriesBar {
  pub fn total(&self) -> TokenUsage {
    self.by_cli.iter().copied().sum()
  }
}

pub(super) fn bars(within: &Within<'_>, range: DayRange, grouping: Grouping) -> Vec<SeriesBar> {
  let buckets = range.buckets(grouping);
  let mut bars: Vec<SeriesBar> = buckets
    .iter()
    .map(|bucket| SeriesBar {
      bucket: *bucket,
      by_cli: [TokenUsage::default(); 3],
      by_account: AccountUsage::default(),
      runs: 0,
    })
    .collect();
  let mut runs: Vec<HashSet<RunId>> = vec![HashSet::new(); bars.len()];
  for turn in &within.turns {
    let Some(index) = bucket_of(&buckets, within.end, turn.at) else {
      continue;
    };
    let series = CliSeries::of(turn.cli());
    bars[index].by_cli[series.index()] += turn.usage;
    bars[index].by_account.add(turn.account(), turn.usage);
    runs[index].insert(turn.run().id);
  }
  for (bar, runs) in bars.iter_mut().zip(runs) {
    bar.runs = runs.len();
  }
  bars
}
