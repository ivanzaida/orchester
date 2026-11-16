//! What the project's Orchestrator conversations watch, as the person reads it (`docs/design/orchestrator-watch.md`):
//! the `/watching` answer (frame 06 `Ff7AD`, [`Watching`]), who leads a module and a task (`i687ei` a and b,
//! [`LedIn`]) and what a conversation leads, for its header chips and its History row (`X93D8T`, `i687ei` c,
//! [`ChatLeads`]). Everything is read from stored state only: the subscriptions that lead their targets (ORC-758),
//! the conversations' titles, the project's default order and its tasks with their latest runs ([`ProjectTasks`]),
//! never from an agent. The words are this crate's [`Message`]s; lists of them are joined by the app. In a project
//! linked to a team server the tasks are the team's, of which this computer keeps no rows: they are read as their
//! runs recorded them, and those that never ran here as the team server's last reading of the watched modules holds
//! them ([`team`], `docs/plan/orchestrator-autonomy/OA-28-…`).

mod answer;
mod counts;
mod leads;
mod led;
mod targets;
mod team;

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::{ChatId, ProjectId, TaskId},
  orchestrator::{Subscription, Subscriptions, WatchTarget, settings::WatchSettings},
  run::TaskPhase,
  task::Task,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

pub use self::{
  answer::{Going, WaitsFor, WatchedSubscription, WatchedTask, Watching},
  counts::WatchedTasks,
  leads::{ChatLeads, LeadChip, LeadCounts, Leader, LedIn},
  led::LedToEnd,
  targets::{TargetView, WatchOrder},
};
use super::team_status::TeamReading;
use crate::{
  messages::Message,
  tasks::{data::ProjectTasks, recorded},
};

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
  /// A linked project's team tasks that `tasks` holds no row of, with where each stands.
  team_tasks: Vec<team::TeamTaskRead>,
  /// What the work led to the end used of its ceilings; empty for reads made without a database.
  led: led::LedReads,
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
      team_tasks: Vec::new(),
      led: led::LedReads::default(),
    }
  }

  /// Reads them from `db`, the project database of `repo_root`. It writes nothing; git tells, as for the task list,
  /// which latest runs that passed every step were merged outside Orchester (by a person, or by a team mate's
  /// computer for a team task), so they read as merged and not as left to merge.
  pub fn read(db: &ProjectDb, repo_root: &Path) -> Result<Self, StoreError> {
    let programs = Programs::processes();
    let mut reads = Self::new(
      db.project().id(),
      db.active_subscriptions()?,
      chat_titles(db)?,
      ProjectTasks::read(db, repo_root, Some(&programs))?,
      &db.orchestrator_watch()?,
    );
    reads.team = TeamReading::of(repo_root);
    if db.sharing()?.is_some() {
      reads.team_tasks = team::read((db, repo_root, &programs), &reads.tasks.tasks, &reads.team)?;
    }
    reads.led = led::LedReads::read(db, &reads.subscriptions)?;
    Ok(reads)
  }

  /// What a conversation's header shows of what it leads ([`Self::chat_leads`]), read from `db`: only the
  /// subscriptions, the conversations' titles, the project's modules and the tasks a subscription names, not the
  /// project's other tasks and no git, so the header shows its chips as soon as the conversation does. A team task that
  /// never ran here is the team server's last reading of it, in `repo_root`'s checkout.
  pub fn read_chat_leads(db: &ProjectDb, repo_root: &Path) -> Result<Self, StoreError> {
    let subscriptions = db.active_subscriptions()?;
    let team = TeamReading::of(repo_root);
    let mut named = Vec::new();
    for subscription in subscriptions.as_slice() {
      let WatchTarget::Task(id) = subscription.target() else {
        continue;
      };
      let found = match recorded::task_of(db, id)? {
        Some(task) => Some(task),
        None => team.task(id).and_then(|seen| team::seen_task(id, seen)),
      };
      named.extend(found);
    }
    let tasks = ProjectTasks {
      modules: Arc::new(db.modules()?),
      tasks: Arc::new(named),
      ..ProjectTasks::default()
    };
    Ok(Self::new(
      db.project().id(),
      subscriptions,
      chat_titles(db)?,
      tasks,
      &db.orchestrator_watch()?,
    ))
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
    WatchTarget::of_task(task.id(), self.team.module_of(task))
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
    self.every_task().find(|task| task.id() == id)
  }

  /// Every task the reads hold: the project's stored tasks, then a linked project's team tasks.
  fn every_task(&self) -> impl Iterator<Item = &Task> {
    self
      .tasks
      .tasks
      .iter()
      .chain(self.team_tasks.iter().map(|read| &read.task))
  }

  /// Whether `task` is Done or Canceled: a team task as the team server last said, any other as stored.
  fn closed(&self, task: &Task) -> bool {
    self
      .team
      .status_of(task.id())
      .map_or_else(|| task.organization().status().is_closed(), |status| status.is_closed())
  }

  fn team_task(&self, id: TaskId) -> Option<&team::TeamTaskRead> {
    self.team_tasks.iter().find(|read| read.task.id() == id)
  }

  /// Where `task` stands; `None` when its latest run cannot be read.
  fn phase(&self, task: &Task) -> Option<TaskPhase> {
    if let Some(read) = self.team_task(task.id()) {
      return read.phase;
    }
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
