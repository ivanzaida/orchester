//! Why no turn of the Orchestrator starts by itself for a while (`docs/plan/orchestrator-autonomy/OA-03-…`): the
//! project's daily budget is used, or the account the Orchestrator runs on is at its plan limit. The events wait
//! meanwhile; their turns start at the reset. A person's own message is not held: its turn is refused or fails as it
//! always did, and says why.

use std::time::Duration;

use crate::time::Timestamp;

/// How long a plan limit holds when its CLI did not say when it lifts: after it the next turn is tried, and a limit
/// still there holds again.
pub const UNKNOWN_RESET: Duration = Duration::from_secs(60 * 60);

/// The Orchestrator's account is at its plan limit, as its CLI reported at `since`; `resets` is when the CLI said the
/// limit lifts, when it said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanHold {
  pub since: Timestamp,
  pub resets: Option<Timestamp>,
}

impl PlanHold {
  /// Until when it holds: the reset the CLI named, or [`UNKNOWN_RESET`] after the report.
  pub fn until(self) -> Timestamp {
    self.resets.unwrap_or_else(|| {
      let wait = i64::try_from(UNKNOWN_RESET.as_millis()).unwrap_or(i64::MAX);
      Timestamp::from_unix_millis(self.since.unix_millis().saturating_add(wait))
    })
  }
}

/// What holds the turns Orchester starts by itself, and until when.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
  /// Today's budget is used: until the next local day starts.
  Budget { until: Timestamp },
  /// The Orchestrator's account is at its plan limit.
  PlanLimit { until: Timestamp },
}

impl Held {
  pub const fn until(self) -> Timestamp {
    match self {
      Self::Budget { until } | Self::PlanLimit { until } => until,
    }
  }

  /// What holds at `now`: the daily budget when it is `budget_used`, else the plan limit `plan` while it has not
  /// lifted.
  pub fn at(now: Timestamp, budget_used: bool, plan: Option<PlanHold>) -> Option<Self> {
    if budget_used {
      return Some(Self::Budget {
        until: next_local_day(now),
      });
    }
    plan
      .map(PlanHold::until)
      .filter(|&until| until > now)
      .map(|until| Self::PlanLimit { until })
  }
}

/// When the local day after `now`'s starts. A day is 23 to 25 hours long, so the start of the day that holds the
/// instant 26 hours after today's start is tomorrow's.
fn next_local_day(now: Timestamp) -> Timestamp {
  const INTO_TOMORROW: i64 = 26 * 60 * 60 * 1000;
  Timestamp::from_unix_millis(now.local_day_start().unix_millis().saturating_add(INTO_TOMORROW)).local_day_start()
}
