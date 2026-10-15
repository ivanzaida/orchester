//! The Usage dashboard (`docs/design/orchester-workspace.md`, "Usage dashboard"): where the tokens of
//! a project's runs, or of every project's, go over a range of days, against the same number of days
//! before. Token counts are what the CLIs reported ("Token budgets"): *spent* = input − cached input +
//! output, *cached* = cached input, *output* includes reasoning; no money anywhere. A turn counts in
//! the hour it completed, so a run that spans the range's edge counts on both sides. A review of auto
//! mode's reviewer counts as a turn of its run, in the hour it ended, on the reviewer's CLI and model
//! and under no agent profile.

mod breakdown;
mod efficiency;
mod expensive;
mod facts;
mod filter;
mod grouped;
pub mod limits;
mod period;
mod report;
mod series;

pub use self::{
  breakdown::{BreakdownKey, BreakdownRow, Breakdowns},
  efficiency::{EditTokens, Efficiency, FailedRuns},
  expensive::{RunCost, TaskCost},
  facts::{IndexGiven, InvocationFacts, ProjectFacts, ReviewUsage, RunFacts, RunOutcome, TurnUsage, UsageFacts},
  filter::{FilterOptions, UsageFilter, toggle},
  grouped::{GroupBy, GroupKey, GroupRow, Grouped, Named, grouped},
  period::{Bucket, DayRange, DayRangeError, Grouping, HOURLY_DAYS, Period, bucket_of, day_start, local_date},
  report::{Summary, UsageQuery, UsageReport},
  series::{Measure, SeriesBar},
};
use crate::connection::AgentProvider;

/// The series a CLI's tokens are drawn in. Color follows the CLI, never its rank: Codex, Claude
/// Code, and a third for any other provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CliSeries {
  Codex,
  Claude,
  Other,
}

impl CliSeries {
  /// Bottom to top in a stacked bar.
  pub const ALL: [Self; 3] = [Self::Codex, Self::Claude, Self::Other];

  pub const fn of(cli: Option<AgentProvider>) -> Self {
    match cli {
      Some(AgentProvider::CodexCli) => Self::Codex,
      Some(AgentProvider::ClaudeCodeCli) => Self::Claude,
      Some(AgentProvider::AnthropicApi | AgentProvider::OpenAiApi) | None => Self::Other,
    }
  }

  pub const fn index(self) -> usize {
    match self {
      Self::Codex => 0,
      Self::Claude => 1,
      Self::Other => 2,
    }
  }
}

/// How a figure changed against the previous period. Never good or bad: only more or less.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
  /// Percent more.
  More(u64),
  /// Percent less.
  Less(u64),
  Same,
  /// The previous period had none, so no percentage says anything.
  NoBaseline,
}

impl Change {
  /// The change from `before` to `now` in whole percent, rounded half up.
  pub fn between(now: u64, before: u64) -> Self {
    if before == 0 {
      return if now == 0 { Self::Same } else { Self::NoBaseline };
    }
    let (now, before) = (u128::from(now), u128::from(before));
    let percent = |difference: u128| u64::try_from((difference * 100 + before / 2) / before).unwrap_or(u64::MAX);
    match now.cmp(&before) {
      std::cmp::Ordering::Greater => Self::More(percent(now - before)),
      std::cmp::Ordering::Less => Self::Less(percent(before - now)),
      std::cmp::Ordering::Equal => Self::Same,
    }
  }

  /// The change of a count from `before` to `now`, in the count's own units (*9 more*).
  pub fn count(now: u64, before: u64) -> Self {
    match now.cmp(&before) {
      std::cmp::Ordering::Greater => Self::More(now - before),
      std::cmp::Ordering::Less => Self::Less(before - now),
      std::cmp::Ordering::Equal => Self::Same,
    }
  }

  /// The change of a share from `before` to `now`, in percentage points; `None` when either share
  /// is unknown.
  pub fn points(now: Option<u64>, before: Option<u64>) -> Option<Self> {
    let (now, before) = (now?, before?);
    Some(match now.cmp(&before) {
      std::cmp::Ordering::Greater => Self::More(now - before),
      std::cmp::Ordering::Less => Self::Less(before - now),
      std::cmp::Ordering::Equal => Self::Same,
    })
  }
}

/// `part` of `whole` in whole percent, rounded half up; `None` when `whole` is zero.
pub fn percent(part: u64, whole: u64) -> Option<u64> {
  if whole == 0 {
    return None;
  }
  let (part, whole) = (u128::from(part), u128::from(whole));
  Some(u64::try_from((part * 100 + whole / 2) / whole).unwrap_or(u64::MAX))
}
