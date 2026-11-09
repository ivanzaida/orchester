//! Subscriptions that end by themselves (ORC-764, `orchester_core::orchestrator::watch::ending`): once every task of a
//! watched module is Done or Canceled, or a watched task is, its subscription ends, and its conversation keeps the line
//! that says so (*Every task of Mobile release is Done — watching ended*) and, with *Let the Orchestrator answer
//! agents* on, one turn that writes the module's or the task's summary and changes nothing (`super::guard`), which
//! `super::waits` starts as a daily report's. The waits follower looks for them each time it follows, so a status
//! changed anywhere (on a task's page, by a merge, by the chat, by the team server) ends them; a task reopened
//! afterwards does not bring an ended subscription back. A team task's status is the team server's: it is read from the
//! last reading of the server ([`super::team_status`]), never from a stored row, and while it cannot be read the
//! subscription goes on.

use std::collections::HashSet;

use orchester_core::{
  id::{ModuleId, ProjectId, TaskId},
  module::Module,
  orchestrator::{
    Subscription, WatchTarget,
    watch::{EndReason, Ending, TargetTasks, WatchSummary},
  },
  store::TaskStore,
  task::{Task, organization::WorkflowStatus},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::team_status::TeamReading;
use crate::{
  links::{Link, TaskPage},
  messages::Message,
};

/// The line a conversation shows for `summary`'s end: what ended it (*Every task of Mobile release is Done*), then
/// [`ended_aside`].
pub fn ended_line(summary: &WatchSummary) -> Message {
  let name = summary.name.clone();
  match summary.reason {
    EndReason::TaskDone => Message::new("orchestrator.watch.ended.taskDone").with("task", name),
    EndReason::TaskCanceled => Message::new("orchestrator.watch.ended.taskCanceled").with("task", name),
    EndReason::ModuleClosed { canceled: 0, .. } => {
      Message::new("orchestrator.watch.ended.moduleDone").with("module", name)
    }
    EndReason::ModuleClosed { done: 0, .. } => {
      Message::new("orchestrator.watch.ended.moduleCanceled").with("module", name)
    }
    EndReason::ModuleClosed { .. } => Message::new("orchestrator.watch.ended.moduleClosed").with("module", name),
  }
}

/// What the end's line adds after [`ended_line`]: *— watching ended*, and, while the turn it starts by itself writes
/// the summary under it, where that summary is.
pub fn ended_aside(summary: &WatchSummary) -> Message {
  Message::new(match (summary.automatic, summary.target) {
    (true, WatchTarget::Task(_)) => "orchestrator.watch.ended.asideTask",
    (true, _) => "orchestrator.watch.ended.asideModule",
    (false, _) => "orchestrator.watch.ended.aside",
  })
}

/// Whether all the work `reason` ended on is Done, which the end's line marks as a success.
pub const fn ended_done(reason: EndReason) -> bool {
  match reason {
    EndReason::TaskDone => true,
    EndReason::TaskCanceled => false,
    EndReason::ModuleClosed { done, canceled } => done > 0 && canceled == 0,
  }
}

/// Ends, at `at`, each subscription of the project in `db` to a module or a task whose work is all Done or Canceled
/// now, and keeps its summary in its conversation; `automatic`: it starts a turn by itself (*Let the Orchestrator
/// answer agents* is on and the Orchestrator has what it runs on). Whether it ended any. A subscription of a task or a
/// module gone from the project is left to the person. `team`: the team server's last reading of the project's team
/// tasks.
pub(super) fn end_closed(
  db: &mut ProjectDb,
  team: TeamReading,
  automatic: bool,
  at: Timestamp,
) -> Result<bool, StoreError> {
  let subscriptions = db.active_subscriptions()?;
  let closable: Vec<&Subscription> = subscriptions
    .as_slice()
    .iter()
    .filter(|subscription| matches!(subscription.target(), WatchTarget::Task(_) | WatchTarget::Module(_)))
    .collect();
  if closable.is_empty() {
    return Ok(false);
  }
  let work = Work {
    project: db.project().id(),
    team_ids: db.team_task_ids()?.into_iter().collect(),
    tasks: db.tasks()?,
    modules: db.modules()?,
    team,
  };
  let mut ended = false;
  for subscription in closable {
    let Some(summary) = work.summary(subscription, automatic) else {
      continue;
    };
    ended |= db.end_subscription_by_itself(subscription, &summary, at)?;
  }
  Ok(ended)
}

/// The project's tasks and modules, as the ending rule reads them. A local task's status is its stored row's. A team
/// task's is the team server's (the stored row of one, a copy the old sharing stream cached, is not read): the last
/// reading in `team`, which has none of a task the server could not be asked about, so its subscription goes on, the
/// same as one whose task is gone.
struct Work {
  project: ProjectId,
  team_ids: HashSet<TaskId>,
  tasks: Vec<Task>,
  modules: Vec<Module>,
  team: TeamReading,
}

/// What an ended subscription's target is called, its title when it is a task, its link, and why it ended.
type Ended = (String, Option<String>, String, EndReason);

impl Work {
  /// The summary `subscription` ends with now; `None` while it goes on, or once its target is gone.
  fn summary(&self, subscription: &Subscription, automatic: bool) -> Option<WatchSummary> {
    let (name, title, link, reason) = match subscription.target() {
      WatchTarget::Task(id) if self.team_ids.contains(&id) => self.team_task_ended(id)?,
      WatchTarget::Task(id) => self.task_ended(id)?,
      WatchTarget::Module(id) => self.module_ended(id)?,
      WatchTarget::WithoutModule | WatchTarget::Project => return None,
    };
    Some(WatchSummary {
      target: subscription.target(),
      name,
      title,
      reason,
      order: subscription.order().map(str::to_owned),
      since: subscription.since(),
      link,
      automatic,
    })
  }

  fn task_ended(&self, id: TaskId) -> Option<Ended> {
    let task = self.tasks.iter().find(|task| task.id() == id)?;
    let reason = ends(TargetTasks::Task(task.organization().status()))?;
    let link = Link::Task(self.project, task.key().clone(), TaskPage::Summary).path();
    let title = task.details().title.as_str().to_owned();
    Some((task.key().to_string(), Some(title), link, reason))
  }

  /// A team task ends its subscription when the server last said it is Done or Canceled.
  fn team_task_ended(&self, id: TaskId) -> Option<Ended> {
    let task = self.team.task(id)?;
    let reason = ends(TargetTasks::Task(task.status))?;
    let link = Link::Task(self.project, task.key.clone(), TaskPage::Summary).path();
    Some((task.key.to_string(), Some(task.title.clone()), link, reason))
  }

  /// A module ends its subscription when all its tasks are closed: its local tasks as stored, and the team tasks the
  /// server puts in it. Until the reading knows them all it goes on: before the first reading, while the server could
  /// not be asked, and for a team task recorded since the reading was made. An ending is not undone.
  fn module_ended(&self, id: ModuleId) -> Option<Ended> {
    let module = self.modules.iter().find(|module| module.id() == id)?;
    if !self.team.knows_module(id, &self.team_ids) {
      return None;
    }
    let local = self
      .tasks
      .iter()
      .filter(|task| task.module() == Some(id) && !self.team_ids.contains(&task.id()))
      .map(|task| task.organization().status());
    let team = self
      .team
      .tasks()
      .filter(|task| task.module == Some(id))
      .map(|task| task.status);
    let statuses: Vec<WorkflowStatus> = local.chain(team).collect();
    let reason = ends(TargetTasks::Module(&statuses))?;
    let link = Link::Module(self.project, id).path();
    Some((module.name().as_str().to_owned(), None, link, reason))
  }
}

/// Why a target whose tasks are `tasks` ends now; `None` while it goes on.
fn ends(tasks: TargetTasks<'_>) -> Option<EndReason> {
  match tasks.ending() {
    Ending::Ends(reason) => Some(reason),
    Ending::Open | Ending::Never(_) => None,
  }
}
