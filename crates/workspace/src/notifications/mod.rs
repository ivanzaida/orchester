//! The one place that decides which OS notifications `orchester-app` shows (ORC-56,
//! `docs/design/orchester-workspace.md`, "Notifications"). Its events come from what the app already follows for every
//! open project: what waits for the person, as every *Needs you* counts it ([`ProjectTasks::waiting`]), how the
//! latest runs ended, as the Orchestrator follows run ends (`docs/design/orchestrator-toolset.md`, decision 5), the
//! project's runner, its merges, the Orchestrator's proposals, and the accounts' plan limits ([`sources`]). What the
//! Orchestrator did itself, such as a merge its chat carried out, is told in its chat and gives no event. The
//! [`Notifier`] turns them into notifications: each event once, only while it still stands, only for a group switched
//! on and outside quiet hours, only when the window is not focused or the place the notification opens is not on
//! screen, and the events of a group within [`COALESCE`] of each other as one notification. The taskbar's or Dock's
//! badge counts what *Needs you* counts. The platform shows them; nothing here talks to the OS.
//!
//! [`ProjectTasks::waiting`]: crate::tasks::data::ProjectTasks::waiting

mod notifier;
mod proposals;
pub mod sources;
mod words;

use std::{sync::Arc, time::Duration};

pub use orchester_core::{
  id::AgentAccountId,
  notification::{NotificationGroup, NotificationSettings, QuietHours},
};
use orchester_core::{
  id::{ChatId, ProjectId},
  project::TaskKey,
};

pub use self::{
  notifier::{Notifier, Scope, View},
  proposals::{ChatReading, WaitingProposal, chats_in, merged_by_chat},
  words::Notification,
};
use crate::{
  links::{Link, TaskPage, with_query},
  tasks::{attention::SummaryCard, list::DecisionWhy},
};

/// How long after a group's notification its next events wait, to be shown together as one.
pub const COALESCE: Duration = Duration::from_secs(30);

/// What happened, by the group it is notified in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
  /// A tool request waits for a decision: a command, or an MCP tool's call.
  Approval { command: bool },
  /// An agent asks a question, `agent` its role.
  Question { agent: Arc<str> },
  /// A step waits for a message without a question that can be read.
  Message,
  /// A plan waits for a decision.
  Plan { version: u32 },
  /// A run waits for a decision on how it goes on.
  Decision(DecisionWhy),
  /// A run stopped at a limit; `None` when the run read does not say which.
  Limit(Option<LimitKind>),
  /// A run was interrupted: the app or the runner crashed, or the runner could not carry it on.
  Interrupted,
  /// The project's runner went away and did not come back.
  RunnerOffline,
  /// The project's runner could not start.
  RunnerNotStarted,
  /// A merge stopped at conflicts.
  MergeConflicts,
  /// A merge stopped where Local integration carries on.
  LocalIntegration,
  /// A run passed every step: its work waits to be merged.
  ReadyToMerge,
  /// A run's work was merged, in Orchester or outside it.
  Merged,
  /// A run failed with no route left.
  Failed,
  /// An account used this much of a plan window.
  PlanLimitNear { percent: u32 },
  /// An account's plan window that was nearly or fully used was reset.
  PlanLimitReset,
  /// An Orchestrator proposal waits for the person.
  Proposal,
}

/// The limit a run stopped at, as people name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LimitKind {
  Tokens,
  Time,
  DailyBudget,
  GroupBudget,
  StepBudget,
  PlanLimit,
}

impl Kind {
  /// The group it is notified in.
  pub const fn group(&self) -> NotificationGroup {
    match self {
      Self::Approval { .. } | Self::Question { .. } | Self::Message | Self::Plan { .. } | Self::Decision(_) => {
        NotificationGroup::NeedsYou
      }
      Self::Limit(_)
      | Self::Interrupted
      | Self::RunnerOffline
      | Self::RunnerNotStarted
      | Self::MergeConflicts
      | Self::LocalIntegration => NotificationGroup::Stopped,
      Self::ReadyToMerge
      | Self::Merged
      | Self::Failed
      | Self::PlanLimitNear { .. }
      | Self::PlanLimitReset
      | Self::Proposal => NotificationGroup::Finished,
    }
  }
}

/// Where an event happened: in a project, or to an agent account, which every project shares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
  Project { id: ProjectId, name: Arc<str> },
  Account { name: Arc<str> },
}

/// The task an event is about, as people name it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventTask {
  pub key: TaskKey,
  pub title: Arc<str>,
}

/// The place a notification opens, by the same route paths as the domain tools' `link` ([`Link`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
  /// A task's page, pointing at a decision card of its Summary when `card` names one.
  Task {
    project: ProjectId,
    key: TaskKey,
    page: TaskPage,
    card: Option<SummaryCard>,
  },
  /// The project's Overview, whose status bar shows its runner.
  Runner(ProjectId),
  /// One of the project's Orchestrator conversations.
  Orchestrator(ProjectId, ChatId),
  /// Settings › Accounts showing an account.
  Account(AgentAccountId),
}

impl Target {
  /// The route path the app's router reads.
  pub fn path(&self) -> String {
    match self {
      Self::Task { project, key, page, .. } => Link::Task(*project, key.clone(), *page).path(),
      Self::Runner(project) => Link::Project(*project).path(),
      Self::Orchestrator(project, chat) => with_query(
        format!("{}/orchestrator", Link::Project(*project).path()),
        &[("chat", Some(&chat.to_string()))],
      ),
      Self::Account(account) => Link::Account(*account).path(),
    }
  }
}

/// One thing the person may be told of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
  /// Names what happened: an event is notified once, and again only after it stopped standing and came back.
  pub key: Arc<str>,
  /// Events that stand under one batch are one notification: the runs the daily budget or a group's budget stopped.
  pub batch: Option<Arc<str>>,
  pub kind: Kind,
  pub origin: Origin,
  pub task: Option<EventTask>,
  /// The step it is about, by its name.
  pub step: Option<Arc<str>>,
  pub target: Target,
}

impl Event {
  pub const fn group(&self) -> NotificationGroup {
    self.kind.group()
  }
}
