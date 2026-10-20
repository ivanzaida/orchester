//! Plan limits history (`docs/design/orchester-workspace.md`, "Usage dashboard", *Plan limits
//! history*): a snapshot is stored whenever a CLI reports its limits during a run or Orchester reads
//! them outside one, and kept 90 days. Snapshots exist only while Orchester runs agents or reads, so
//! the history is honest about gaps: points of one window are joined while reports keep coming, a
//! stretch without reports is a gap (use outside Orchester is unknown until the next report), a window
//! that reset between two reports is not joined across the reset, nothing is drawn before a CLI's
//! first report, and no value is invented.

use std::{collections::BTreeMap, sync::Arc};

use thiserror::Error;

use super::{CliSeries, UsageFilter};
use crate::{
  account::AccountCli,
  connection::{
    AgentProvider,
    plan_limits::{PlanLimits, ReadingSource},
  },
  id::AgentAccountId,
  run::RunNumber,
  time::Timestamp,
};

/// How long snapshots are kept.
pub const RETENTION_DAYS: i64 = 90;
/// Longer than this without a report between two snapshots of a window is a gap: turns report
/// limits every few minutes while an agent works, so an hour without one means no agent worked.
pub const GAP_MILLIS: i64 = 60 * 60 * 1000;
const DAY_MILLIS: i64 = 24 * 60 * 60 * 1000;

/// The oldest time a snapshot stored at `now` keeps others from.
pub const fn retained_since(now: Timestamp) -> Timestamp {
  Timestamp::from_unix_millis(now.unix_millis() - RETENTION_DAYS * DAY_MILLIS)
}

/// The run whose turn reported a snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotSource {
  pub project: Arc<str>,
  pub task_key: Arc<str>,
  pub run: RunNumber,
}

/// One stored report of a CLI's limits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LimitSnapshot {
  pub provider: AgentProvider,
  /// The account the CLI ran on; `None` for a report from before accounts, which was Default's.
  pub account: Option<crate::id::AgentAccountId>,
  pub at: Timestamp,
  pub limits: PlanLimits,
  /// The run whose turn reported it; `None` for a read, and when the run is no longer known.
  pub source: Option<SnapshotSource>,
  /// Whether a turn reported it or Orchester read it outside a run.
  pub origin: ReadingSource,
}

/// How a window is drawn: weekly windows solid, 5-hour windows dashed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WindowKind {
  FiveHour,
  Weekly,
  Other,
}

impl WindowKind {
  pub const fn of(minutes: u64) -> Self {
    match minutes {
      300 => Self::FiveHour,
      10_080 => Self::Weekly,
      _ => Self::Other,
    }
  }
}

/// One reading of a window, in percent used.
#[derive(Clone, Debug, PartialEq)]
pub struct LimitPoint {
  pub at: Timestamp,
  pub percent: f64,
  pub resets: Option<Timestamp>,
  pub source: Option<SnapshotSource>,
  pub origin: ReadingSource,
}

/// How two consecutive points of a window are joined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Join {
  /// Reports kept coming: a solid line.
  Solid,
  /// A stretch without reports: a dotted 1px line.
  Gap,
  /// The window reset between them: not joined.
  Reset,
}

/// One window's points in the range.
#[derive(Clone, Debug, PartialEq)]
pub struct LimitLine {
  pub provider: AgentProvider,
  /// The account whose limits these are; `None` for a provider without accounts.
  pub account: Option<AgentAccountId>,
  pub minutes: u64,
  pub points: Vec<LimitPoint>,
  /// Between each point and the next.
  pub joins: Vec<Join>,
  /// When the window reset within the range, as the reports before each reset said.
  pub resets: Vec<Timestamp>,
}

impl LimitLine {
  pub const fn kind(&self) -> WindowKind {
    WindowKind::of(self.minutes)
  }

  pub const fn series(&self) -> CliSeries {
    CliSeries::of(Some(self.provider))
  }
}

/// A window's last known reading, for the legend and the no-runs state.
#[derive(Clone, Debug, PartialEq)]
pub struct CurrentLimit {
  pub provider: AgentProvider,
  pub account: Option<AgentAccountId>,
  pub minutes: u64,
  /// `None` when the CLI reported only the window's state.
  pub percent: Option<f64>,
  pub resets: Option<Timestamp>,
  pub at: Timestamp,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LimitHistory {
  pub lines: Vec<LimitLine>,
  /// The latest reading of each window reported before the range's end.
  pub current: Vec<CurrentLimit>,
  /// Each CLI's first stored report: before it, nothing is known.
  pub first: Vec<(AgentProvider, Timestamp)>,
  /// Each account's first stored report, in the order first reported.
  pub first_by_account: Vec<(AgentProvider, Option<AgentAccountId>, Timestamp)>,
}

impl LimitHistory {
  /// The history of the CLIs and accounts `filter` admits between `start` and `end`, measured at `now`: one
  /// line per account and window. A report from before accounts were recorded is its CLI's Default account's.
  pub fn of(
    snapshots: &[LimitSnapshot],
    (start, end): (Timestamp, Timestamp),
    now: Timestamp,
    filter: &UsageFilter,
  ) -> Self {
    let mut sorted: Vec<&LimitSnapshot> = snapshots
      .iter()
      .filter(|snapshot| {
        filter.admits_cli(snapshot.provider) && filter.admits_account(account_of(snapshot)) && snapshot.at < end
      })
      .collect();
    sorted.sort_by_key(|snapshot| snapshot.at);
    let mut first: Vec<(AgentProvider, Timestamp)> = Vec::new();
    let mut first_by_account: Vec<(AgentProvider, Option<AgentAccountId>, Timestamp)> = Vec::new();
    let mut windows: BTreeMap<WindowKey, (AgentProvider, Vec<LimitPoint>)> = BTreeMap::new();
    let mut current: BTreeMap<WindowKey, CurrentLimit> = BTreeMap::new();
    for snapshot in sorted {
      let account = account_of(snapshot);
      if !first.iter().any(|(provider, _)| *provider == snapshot.provider) {
        first.push((snapshot.provider, snapshot.at));
      }
      if !first_by_account
        .iter()
        .any(|(provider, of, _)| *provider == snapshot.provider && *of == account)
      {
        first_by_account.push((snapshot.provider, account, snapshot.at));
      }
      for window in &snapshot.limits.windows {
        let key = (CliSeries::of(Some(snapshot.provider)), account, window.minutes());
        let at = window.reported_at().unwrap_or(snapshot.at);
        current.insert(
          key,
          CurrentLimit {
            provider: snapshot.provider,
            account,
            minutes: window.minutes(),
            percent: window.percent(),
            resets: window.resets(),
            at,
          },
        );
        let Some(percent) = window.percent().filter(|_| start <= at) else {
          continue;
        };
        let point = LimitPoint {
          at,
          percent,
          resets: window.resets(),
          source: snapshot.source.clone(),
          origin: snapshot.origin,
        };
        windows
          .entry(key)
          .or_insert_with(|| (snapshot.provider, Vec::new()))
          .1
          .push(point);
      }
    }
    let until = now.min(end);
    let lines = windows
      .into_iter()
      .map(|((_, account, minutes), (provider, points))| line((provider, account, minutes), points, until))
      .collect();
    Self {
      lines,
      current: current.into_values().collect(),
      first,
      first_by_account,
    }
  }

  /// No CLI has reported its limits: the chart says so instead of drawing.
  pub fn is_empty(&self) -> bool {
    self.current.is_empty()
  }
}

/// A window: its CLI, its account and its length.
type WindowKey = (CliSeries, Option<AgentAccountId>, u64);

/// The account a report is of: the one recorded, or, from before accounts were, its CLI's Default.
fn account_of(snapshot: &LimitSnapshot) -> Option<AgentAccountId> {
  snapshot
    .account
    .or_else(|| AccountCli::of(snapshot.provider).map(AccountCli::default_account))
}

fn line(
  (provider, account, minutes): (AgentProvider, Option<AgentAccountId>, u64),
  points: Vec<LimitPoint>,
  until: Timestamp,
) -> LimitLine {
  let mut joins = Vec::new();
  let mut resets = Vec::new();
  for pair in points.windows(2) {
    let (before, after) = (&pair[0], &pair[1]);
    let reset = before.resets.filter(|reset| *reset <= after.at);
    joins.push(match reset {
      Some(reset) => {
        resets.push(reset);
        Join::Reset
      }
      None if after.at.unix_millis() - before.at.unix_millis() > GAP_MILLIS => Join::Gap,
      None => Join::Solid,
    });
  }
  if let Some(reset) = points
    .last()
    .and_then(|last| last.resets)
    .filter(|reset| *reset <= until)
  {
    resets.push(reset);
  }
  resets.dedup();
  LimitLine {
    provider,
    account,
    minutes,
    points,
    joins,
    resets,
  }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("a warning threshold is a whole percentage from 1 to 100")]
pub struct InvalidThreshold;

/// Where the dashed warning line is drawn: a whole percentage of a window, 1–100, saved for the
/// application (limits belong to accounts, not projects).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WarningThreshold(u8);

impl WarningThreshold {
  pub const DEFAULT: Self = Self(80);

  pub fn new(percent: u32) -> Result<Self, InvalidThreshold> {
    match u8::try_from(percent) {
      Ok(percent @ 1..=100) => Ok(Self(percent)),
      _ => Err(InvalidThreshold),
    }
  }

  /// A typed value: whole digits only.
  pub fn parse(text: &str) -> Result<Self, InvalidThreshold> {
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
      return Err(InvalidThreshold);
    }
    text.parse::<u32>().map_err(|_| InvalidThreshold).and_then(Self::new)
  }

  pub const fn percent(self) -> u8 {
    self.0
  }
}

impl Default for WarningThreshold {
  fn default() -> Self {
    Self::DEFAULT
  }
}
