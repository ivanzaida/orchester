//! The end-of-day report of a project (ORC-47): what was done on one local day and how the work stands, kept as an
//! entry of the project's Orchestrator conversation at the time the person chose, and read again by
//! `get_daily_report`. Only facts the project's database holds go into it; `orchester_workspace::orchestrator::report`
//! has the rules that pick them. Every item links to its place in the app.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::wait::RunEnd;
use crate::{
  id::{AgentAccountId, RunId},
  notification::MINUTES_PER_DAY,
  time::Timestamp,
};

/// The time of day the report comes at, in minutes after local midnight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReportTime(u16);

impl ReportTime {
  /// The time a report comes at when the person turns it on and chooses none: 18:00.
  pub const DEFAULT: Self = Self(18 * 60);

  /// `minute` of the day; `None` for one past the day's last.
  pub const fn new(minute: u16) -> Option<Self> {
    if minute < MINUTES_PER_DAY {
      Some(Self(minute))
    } else {
      None
    }
  }

  pub const fn minute(self) -> u16 {
    self.0
  }

  /// The instant it is on local day `day`; a time a DST change skips is the first instant after it.
  pub fn on(self, day: NaiveDate) -> Timestamp {
    let time = chrono::NaiveTime::from_hms_opt(u32::from(self.0 / 60), u32::from(self.0 % 60), 0)
      .expect("a minute of the day is a time of day");
    let local = day.and_time(time);
    // A skipped local time has no instant: the hour after it is the earliest that exists.
    let instant = local
      .and_local_timezone(chrono::Local)
      .earliest()
      .or_else(|| {
        (local + chrono::Duration::hours(1))
          .and_local_timezone(chrono::Local)
          .earliest()
      })
      .map_or_else(|| local.and_utc().timestamp_millis(), |at| at.timestamp_millis());
    Timestamp::from_unix_millis(instant)
  }
}

/// When the person wants the report: each day at `time`, for the days whose time came after `since`, when they turned
/// it on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReportSchedule {
  pub time: ReportTime,
  pub since: Timestamp,
}

/// One day's report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyReport {
  /// The local day it is about.
  #[serde(with = "local_day")]
  pub day: NaiveDate,
  /// When it was read: what stands (waits, work, the queue) stands as of then.
  pub built: Timestamp,
  /// The tasks whose run was merged that day, in the order they were merged.
  pub merged: Vec<MergedTask>,
  /// Tasks' latest runs that succeeded and wait to be merged.
  pub ready: Vec<ReportRun>,
  /// Tasks' latest runs that failed, stopped at a limit, wait for a decision or were interrupted, with how.
  pub stopped: Vec<StoppedRun>,
  /// Tasks' latest runs still at work.
  pub working: Vec<ReportRun>,
  /// What waits for the person: tool requests, questions, messages, plans and the Orchestrator's proposals.
  pub waiting: Vec<PersonWait>,
  /// What the Orchestrator did that day, itself or applied by the person, in the order it was done.
  pub actions: Vec<ReportAction>,
  pub usage: DayUsage,
  /// The queued tasks whose run has not started, in the order they are queued.
  pub queued: Vec<ReportTask>,
  /// It starts one Orchestrator turn by itself, which adds a summary: *Let the Orchestrator answer agents* was on when
  /// it was kept.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub automatic: bool,
}

impl DailyReport {
  /// The key the report of `day` is kept by: one report per project and day, whichever conversation keeps it.
  pub fn key_of(day: NaiveDate) -> String {
    format!("report:{}", day.format("%Y-%m-%d"))
  }
}

/// A task, by its key as people read it (`KON-41`) and its title, linking to its page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportTask {
  pub task: String,
  pub title: String,
  pub link: String,
}

/// A task whose run `number` was merged at `at`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergedTask {
  pub task: String,
  pub title: String,
  pub number: u32,
  pub at: Timestamp,
  pub link: String,
}

/// A task's latest run, with the step it is at or stopped at, and since when it stands so, when the run tells.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportRun {
  pub task: String,
  pub title: String,
  pub run: RunId,
  pub number: u32,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub step: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub since: Option<Timestamp>,
  /// The task's Summary, where the person decides how it goes on.
  pub link: String,
}

/// A run that stopped, and how: `end` tells why, and what the person can do follows from it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoppedRun {
  pub run: ReportRun,
  pub end: RunEnd,
}

/// What waits for the person.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonWaitKind {
  /// A tool request of an agent.
  Approval,
  /// An agent's question with suggested answers.
  Question,
  /// A step waiting for a message.
  Message,
  /// A plan waiting for a decision.
  Plan,
  /// A proposal of the Orchestrator waiting to be applied or dismissed.
  Proposal,
}

/// One thing that waits for the person: of `kind`, in run `number` of `task` when it is a run's, about `what` (the
/// command of a tool request, the waiting step of a message or a plan, the tool of a proposal), since `since` when
/// that is kept, linking to its card.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonWait {
  pub kind: PersonWaitKind,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub task: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub number: Option<u32>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub what: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub since: Option<Timestamp>,
  pub link: String,
}

/// How an action of the Orchestrator was carried out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionWay {
  /// The Orchestrator did it itself.
  Direct,
  /// The person applied what it proposed.
  Applied,
}

/// An action of the Orchestrator through tool `tool`, on `task` or on `subject` (a draft's name), done `way` at `at`,
/// linking to what it acted on, or to its conversation when it names nothing with a page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportAction {
  pub tool: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub task: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub subject: Option<String>,
  pub way: ActionWay,
  pub at: Timestamp,
  pub link: String,
}

/// The day's tokens (spent: input − cached input + output) and active agent time, as `get_usage` reads them, against
/// the project's daily budget (`None`: no daily limit), by task and by account, most spent first, linking to the
/// Usage page of the day.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayUsage {
  pub spent: u64,
  pub active_ms: u64,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub budget: Option<u64>,
  pub tasks: Vec<TaskShare>,
  pub accounts: Vec<AccountShare>,
  pub link: String,
}

/// A task's share of the day's usage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskShare {
  pub task: String,
  pub title: String,
  pub spent: u64,
  pub active_ms: u64,
  pub link: String,
}

/// An account's share of the day's usage; `account` is `None` for usage that started on no account, and `name` is
/// the account's name when it has one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountShare {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub account: Option<AgentAccountId>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
  pub spent: u64,
  pub active_ms: u64,
  pub link: String,
}

/// A local day as `2026-10-20`.
mod local_day {
  use chrono::NaiveDate;
  use serde::{Deserialize, Deserializer, Serializer, de::Error};

  const FORMAT: &str = "%Y-%m-%d";

  pub(super) fn serialize<S: Serializer>(day: &NaiveDate, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(&day.format(FORMAT))
  }

  pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<NaiveDate, D::Error> {
    let text = String::deserialize(deserializer)?;
    NaiveDate::parse_from_str(&text, FORMAT).map_err(D::Error::custom)
  }
}
