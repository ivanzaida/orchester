//! What the /watch pickers offer (`docs/design/orchestrator-watch.md`, frames 02 `c8pALl` and 03 `mQdxa`): the
//! project's modules ([`modules`]) and tasks ([`tasks`]) that match what the person typed after the command, each
//! with who leads it already: another conversation, named by its title and opened by its id, this one, or the
//! command being written, which just added it. A target whose subscription would end at once
//! ([`TargetTasks::ending`]) is offered faded with the reason: a module whose every task is Done or Canceled
//! (*Nothing left to watch*), a Done or Canceled task. Plain data: the caller reads the project's tasks and its
//! [`Leaders`] off the UI thread, and asks again as the person types.

pub mod modules;
pub mod tasks;

use std::{collections::HashMap, sync::Arc};

use orchester_core::{
  id::{ChatId, ModuleId, TaskId},
  module::Module,
  orchestrator::{
    Subscriptions, WatchTarget,
    watch::{EndReason, Ending, TargetTasks},
  },
  project::TaskKey,
  run::TaskPhase,
  task::organization::WorkflowStatus,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::tasks::{data::ProjectTasks, list::Latest};

/// Who leads a target the pickers offer, as its row says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Leader {
  /// No conversation leads it.
  Nobody,
  /// The command being written chose it already.
  JustAdded,
  /// This conversation leads it; `via` names the less specific target it comes here through (a task's module, the
  /// tasks without a module, the project), `None` when it is led itself.
  Here { via: Option<WatchTarget> },
  /// Another conversation leads it, `chat` titled `title` (`None`: no conversation of the project has that id now);
  /// `via` as for [`Leader::Here`].
  Elsewhere {
    chat: ChatId,
    title: Option<Arc<str>>,
    via: Option<WatchTarget>,
  },
}

/// The conversation the command is written in, and the targets it chose so far.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choosing<'a> {
  pub chat: ChatId,
  pub added: &'a [WatchTarget],
}

/// The project's subscriptions that lead their targets, with the titles of its conversations.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Leaders {
  subscriptions: Subscriptions,
  titles: HashMap<ChatId, Arc<str>>,
}

impl Leaders {
  pub fn new(subscriptions: Subscriptions, titles: impl IntoIterator<Item = (ChatId, Arc<str>)>) -> Self {
    Self {
      subscriptions,
      titles: titles.into_iter().collect(),
    }
  }

  /// The subscriptions `db` keeps that lead, and its conversations' titles as History shows them. Blocks on SQLite.
  pub fn read(db: &ProjectDb) -> Result<Self, StoreError> {
    let titles = db
      .chats()?
      .into_iter()
      .map(|listing| (listing.chat.id(), Arc::from(listing.chat.title())));
    Ok(Self::new(db.active_subscriptions()?, titles))
  }

  /// Who leads `target`, else the first of `outer`, the less specific targets that cover it, nearest first.
  fn of(&self, target: WatchTarget, outer: &[WatchTarget], choosing: Choosing<'_>) -> Leader {
    if choosing.added.contains(&target) {
      return Leader::JustAdded;
    }
    let Some(led) = std::iter::once(&target)
      .chain(outer)
      .find_map(|target| self.subscriptions.of(*target))
    else {
      return Leader::Nobody;
    };
    let via = (led.target() != target).then_some(led.target());
    if led.chat() == choosing.chat {
      return Leader::Here { via };
    }
    Leader::Elsewhere {
      chat: led.chat(),
      title: self.titles.get(&led.chat()).cloned(),
      via,
    }
  }
}

/// A task as the pickers count and show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchTask {
  pub id: TaskId,
  pub key: TaskKey,
  pub title: Arc<str>,
  pub module: Option<ModuleId>,
  pub status: WorkflowStatus,
  /// Its latest run is running, with no step waiting for the person.
  pub running: bool,
  /// Something of it waits for the person ([`ProjectTasks::waiting_tasks`]).
  pub waits: bool,
  /// When it last changed, which orders the task picker.
  pub at: Timestamp,
}

/// What the pickers choose from: the project's modules, its tasks and who leads what.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WatchCatalog {
  /// Oldest first, closed ones too, which name their tasks' module.
  pub modules: Vec<Module>,
  pub tasks: Vec<WatchTask>,
  pub leaders: Leaders,
}

impl WatchCatalog {
  /// The catalog of `project`, the tasks the project's pages read, with `leaders`.
  pub fn of(project: &ProjectTasks, leaders: Leaders) -> Self {
    let waiting = project.waiting_tasks();
    let tasks = project
      .tasks
      .iter()
      .map(|task| WatchTask {
        id: task.id(),
        key: task.key().clone(),
        title: task.details().title.as_str().into(),
        module: task.module(),
        status: task.organization().status(),
        running: project.phases.get(&task.id()).and_then(Latest::phase) == Some(TaskPhase::Running),
        waits: waiting.contains(&task.id()),
        at: project.updated.get(&task.id()).copied().unwrap_or(task.created_at()),
      })
      .collect();
    Self {
      modules: project.modules.to_vec(),
      tasks,
      leaders,
    }
  }

  fn module(&self, id: ModuleId) -> Option<&Module> {
    self.modules.iter().find(|module| module.id() == id)
  }
}

/// Why a target is offered faded: watching it would end at once, for this reason.
fn faded(target: TargetTasks<'_>) -> Option<EndReason> {
  match target.ending() {
    Ending::Ends(reason) => Some(reason),
    Ending::Open | Ending::Never(_) => None,
  }
}
