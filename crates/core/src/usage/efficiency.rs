//! *Efficiency*: the cache share per day, the tokens agents used before their first edit with and
//! without the code index, what every agent invocation did with the index ([`IndexUse`]), and the runs that failed.

use chrono::NaiveDate;

use super::{
  IndexGiven, IndexUse, RunCost, Summary,
  facts::InvocationFacts,
  period::{DayRange, Grouping, bucket_of},
  report::Within,
};
use crate::run::TokenUsage;

/// *Tokens before first edit* for one group of invocations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditTokens {
  /// The median spent before the first edit of the invocations that changed a file.
  pub median: Option<u64>,
  /// Invocations that changed a file.
  pub edited: usize,
  /// Invocations that ended without changing a file: left out of the median, and counted.
  pub without_edit: usize,
}

/// *Failed runs*: runs whose latest attempt failed, with what they spent in the range.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FailedRuns {
  pub runs: usize,
  pub of_runs: usize,
  pub spent: u64,
  pub of_spent: u64,
  /// The failed run that spent the most.
  pub largest: Option<RunCost>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Efficiency {
  /// Each day's cache share in whole percent; `None` for a day without input.
  pub cache_by_day: Vec<(NaiveDate, Option<u64>)>,
  pub with_index: EditTokens,
  pub without_index: EditTokens,
  pub index_use: IndexUse,
  pub failed: FailedRuns,
}

impl Efficiency {
  pub(super) fn of(within: &Within<'_>, range: DayRange, summary: &Summary) -> Self {
    let largest = super::expensive::runs(within)
      .into_iter()
      .find(|run| run.outcome.failed() && run.usage.counted() > 0);
    Self {
      cache_by_day: cache_by_day(within, range),
      with_index: edit_tokens(within, IndexGiven::Yes),
      without_index: edit_tokens(within, IndexGiven::No),
      index_use: IndexUse::of(within),
      failed: FailedRuns {
        runs: summary.failed,
        of_runs: summary.runs,
        spent: summary.failed_spent,
        of_spent: summary.spent(),
        largest,
      },
    }
  }
}

fn cache_by_day(within: &Within<'_>, range: DayRange) -> Vec<(NaiveDate, Option<u64>)> {
  let buckets = range.buckets(Grouping::Day);
  let mut days = vec![TokenUsage::default(); buckets.len()];
  for turn in &within.turns {
    if let Some(index) = bucket_of(&buckets, within.end, turn.at) {
      days[index] += turn.usage;
    }
  }
  buckets
    .iter()
    .zip(days)
    .map(|(bucket, usage)| (bucket.date, super::percent(usage.cached_input, usage.input)))
    .collect()
}

/// The agent invocations that started in the range and got the code index as `given`.
fn edit_tokens(within: &Within<'_>, given: IndexGiven) -> EditTokens {
  let started =
    |invocation: &InvocationFacts| within.start <= invocation.started_at && invocation.started_at < within.end;
  let mut before: Vec<u64> = Vec::new();
  let mut without_edit = 0;
  for (launch, _) in &within.launches {
    let invocation = launch.invocation;
    let Some(edit) = invocation
      .edit
      .as_ref()
      .filter(|_| invocation.index == given && started(invocation))
    else {
      continue;
    };
    if edit.first_edit.is_some() {
      before.push(edit.before.counted());
    } else if invocation.ended_at.is_some() {
      without_edit += 1;
    }
  }
  EditTokens {
    median: median(&mut before),
    edited: before.len(),
    without_edit,
  }
}

/// The middle value, or the mean of the two middle ones rounded down.
pub(super) fn median(values: &mut [u64]) -> Option<u64> {
  if values.is_empty() {
    return None;
  }
  values.sort_unstable();
  let middle = values.len() / 2;
  Some(if values.len() % 2 == 1 {
    values[middle]
  } else {
    values[middle - 1] / 2 + values[middle] / 2 + (values[middle - 1] % 2 + values[middle] % 2) / 2
  })
}
