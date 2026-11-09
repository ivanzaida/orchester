//! What the project's Orchestrator conversations watch, as the person reads it (`docs/design/orchestrator-watch.md`):
//! the `/watching` answer (frame 06 `Ff7AD`, [`Watching`]), who leads a module and a task (`i687ei` a and b,
//! [`LedIn`]) and what a conversation leads, for its header chips and its History row (`X93D8T`, `i687ei` c,
//! [`ChatLeads`]). Everything is read from stored state only: the subscriptions that lead their targets (ORC-758),
//! the conversations' titles, the project's default order and its tasks with their latest runs ([`ProjectTasks`]),
//! never from an agent. The words are this crate's [`Message`]s; lists of them are joined by the app.

mod answer;
mod counts;
mod leads;
mod targets;

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::{ChatId, ProjectId, TaskId},
  orchestrator::{Subscription, Subscriptions, WatchTarget, settings::WatchSettings},
  run::TaskPhase,
  task::Task,
};
use orchester_store::{ProjectDb, StoreError};

pub use self::{
  answer::{Going, WaitsFor, WatchedSubscription, WatchedTask, Watching},
  counts::WatchedTasks,
  leads::{ChatLeads, LeadChip, LeadCounts, Leader, LedIn},
  targets::{TargetView, WatchOrder},
};
use super::team_status::TeamReading;
use crate::{messages::Message, tasks::data::ProjectTasks};

/// What the watch reads are made of, read together.
#[derive(Clone, Debug)]
pub struct WatchReads {
  project: ProjectId,
  subscriptions: Subscriptions,
  /// Every conversation's id with its title.
  chats: Vec<(ChatId, Arc<str>)>,
  tasks: ProjectTasks,
  default_order: Arc<str>,
  /// What the team server last said of the team tasks the subscriptions watch; empty for reads made without a folder.
  team: TeamReading,
}

impl WatchReads {
  /// The reads of project `project` from what an app holds already: the subscriptions that lead their targets, the
  /// conversations with their titles, the project's tasks and its watch settings.
  pub fn new(
    project: ProjectId,
    subscriptions: Subscriptions,
    chats: impl IntoIterator<Item = (ChatId, Arc<str>)>,
    tasks: ProjectTasks,
    watch: &WatchSettings,
  ) -> Self {
    Self {
      project,
      subscriptions,
      chats: chats.into_iter().collect(),
      tasks,
      default_order: watch.default_order().into(),
      team: TeamReading::default(),
    }
  }

  /// Reads them from `db`, the project database of `repo_root`. It writes nothing and runs no git: a run merged
  /// outside Orchester reads as its stored phase.
  pub fn read(db: &ProjectDb, repo_root: &Path) -> Result<Self, StoreError> {
    let mut reads = Self::new(
      db.project().id(),
      db.active_subscriptions()?,
      chat_titles(db)?,
      ProjectTasks::read(db, repo_root, None)?,
      &db.orchestrator_watch()?,
    );
    reads.team = TeamReading::of(repo_root);
    Ok(reads)
  }

  /// Who leads `task`, read from `db` ([`Self::task_led_in`]): only the subscriptions, the conversations' titles and
  /// the project's modules, not the project's other tasks, so the task page can read it with the task.
  pub fn read_task_led_in(db: &ProjectDb, task: &Task) -> Result<LedIn, StoreError> {
    let tasks = ProjectTasks {
      modules: Arc::new(db.modules()?),
      tasks: Arc::new(vec![task.clone()]),
      ..ProjectTasks::default()
    };
    let reads = Self::new(
      db.project().id(),
      db.active_subscriptions()?,
      chat_titles(db)?,
      tasks,
      &WatchSettings::default(),
    );
    Ok(reads.task_led_in(task.id()))
  }

  /// The subscription an event of `task` goes to: the most specific one that covers it.
  fn leading(&self, task: &Task) -> Option<&Subscription> {
    WatchTarget::of_task(task.id(), task.module())
      .into_iter()
      .find_map(|target| self.subscriptions.of(target))
  }

  /// The title of `chat`. A subscription's conversation is always kept while it leads (deleting a conversation ends
  /// its subscriptions), so an empty title only shows a conversation deleted after these reads.
  fn title(&self, chat: ChatId) -> Arc<str> {
    self
      .chats
      .iter()
      .find(|(id, _)| *id == chat)
      .map_or_else(|| Arc::from(""), |(_, title)| title.clone())
  }

  fn task(&self, id: TaskId) -> Option<&Task> {
    self.tasks.tasks.iter().find(|task| task.id() == id)
  }

  /// Where `task` stands; `None` when its latest run cannot be read.
  fn phase(&self, task: &Task) -> Option<TaskPhase> {
    self
      .tasks
      .phases
      .get(&task.id())
      .map_or_else(|| Some(TaskPhase::of(task, None, None)), |latest| latest.phase())
  }
}

/// Every conversation of `db` with its title.
fn chat_titles(db: &ProjectDb) -> Result<Vec<(ChatId, Arc<str>)>, StoreError> {
  Ok(
    db.chats()?
      .into_iter()
      .map(|listing| (listing.chat.id(), Arc::from(listing.chat.title())))
      .collect(),
  )
}

/// `count` in the words of `one` or `other`.
fn counted(count: usize, (one, other): (&'static str, &'static str)) -> Message {
  Message::new(if count == 1 { one } else { other }).with("count", count.to_string())
}
