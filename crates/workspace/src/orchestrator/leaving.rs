//! Before you leave (`docs/plan/orchestrator-autonomy/OA-19-…`): what would stop or weaken the work a conversation
//! leads to the end while nobody is at the computer, read when it is asked for, so the person learns it before they
//! go and not the next morning. It warns and never refuses: the person may lead anyway. Each line names what is so
//! and, where a task or a module is the place to fix it, that place; a line the app cannot check is named as not
//! checked rather than left out.
//!
//! Read: the switch, the Orchestrator's connection, the pipelines of the tasks that are still to start, whether
//! those tasks' merges are checked on the merged result (a Deliver step), whether they land in the person's checkout
//! while it has changes of its own, the daily budget, and whether the computer may sleep. Not read here: whether an
//! MCP server a task's profile chooses is on and its program found (the start's own readiness says that, task by
//! task), and whether a server would answer.

use std::path::Path;

use orchester_core::{
  id::{ChatId, ResourceId},
  integration::{base::BaseChoice, workspace::Workspaces},
  orchestrator::{Subscription, WatchTarget},
  resource::Pipeline,
  store::{ResourceStore, SettingsStore},
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use super::{settings::OrchestratorSetup, standing::Reads};
use crate::{
  messages::{self, Message},
  tasks::{changes::GIT, recorded},
};

/// What the app is set to, which the project's database does not hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppSet {
  /// *Let the Orchestrator drive* is on for the project.
  pub drives: bool,
  /// *Keep the computer awake while agents work* is on.
  pub keeps_awake: bool,
}

/// One thing that would stop or weaken the work while nobody is there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Weak {
  /// *Let the Orchestrator drive* is off: no turn starts by itself.
  DriveOff,
  /// The Orchestrator has nothing it can run on, as the message says.
  OrchestratorNotReady(Message),
  /// These tasks have no pipeline, or one that is not published: they cannot start.
  NoPipeline(Vec<String>),
  /// These tasks' pipelines end in no Deliver step: their merge is not checked on the merged result. `pipelines` are
  /// those pipelines, where a Deliver step is added.
  NoDeliverStep {
    tasks: Vec<String>,
    pipelines: Vec<PipelineRef>,
  },
  /// The work merges into the branch the person's checkout has, and the checkout has changes of its own now: a merge
  /// they stand in the way of waits for the person.
  CheckoutHasChanges { files: usize },
  /// The project has no daily budget: nothing bounds what is spent.
  NoDailyBudget,
  /// The computer may go to sleep while agents work.
  MaySleep,
  /// Closing the window stops the work.
  StopsWithWindow,
}

impl Weak {
  /// The name a tool tells it by.
  pub const fn name(&self) -> &'static str {
    match self {
      Self::DriveOff => "drive_off",
      Self::OrchestratorNotReady(_) => "orchestrator_not_ready",
      Self::NoPipeline(_) => "no_pipeline",
      Self::NoDeliverStep { .. } => "no_deliver_step",
      Self::CheckoutHasChanges { .. } => "checkout_has_changes",
      Self::NoDailyBudget => "no_daily_budget",
      Self::MaySleep => "may_sleep",
      Self::StopsWithWindow => "stops_with_window",
    }
  }

  /// The tasks it names, by their keys.
  pub fn tasks(&self) -> &[String] {
    match self {
      Self::NoPipeline(tasks) | Self::NoDeliverStep { tasks, .. } => tasks,
      _ => &[],
    }
  }

  /// What a person reads of it.
  pub fn message(&self) -> Message {
    match self {
      Self::DriveOff => Message::new("orchestrator.leaving.driveOff"),
      Self::OrchestratorNotReady(why) => why.clone(),
      Self::NoPipeline(tasks) => Message::new("orchestrator.leaving.noPipeline").with("tasks", tasks.join(", ")),
      Self::NoDeliverStep { tasks, .. } => {
        Message::new("orchestrator.leaving.noDeliverStep").with("tasks", tasks.join(", "))
      }
      Self::CheckoutHasChanges { files } => {
        Message::new("orchestrator.leaving.checkoutHasChanges").with("count", files.to_string())
      }
      Self::NoDailyBudget => Message::new("orchestrator.leaving.noDailyBudget"),
      Self::MaySleep => Message::new("orchestrator.leaving.maySleep"),
      Self::StopsWithWindow => Message::new("orchestrator.leaving.stopsWithWindow"),
    }
  }
}

/// A pipeline as a line names it, with the id its page opens by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineRef {
  pub id: ResourceId,
  pub name: String,
}

/// What the app cannot tell from here, and says so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotChecked {
  /// Whether the MCP servers the tasks' profiles choose are on, found and answer.
  Servers,
}

impl NotChecked {
  /// What a person reads of it.
  pub fn message(self) -> Message {
    match self {
      Self::Servers => Message::new("orchestrator.leaving.serversNotChecked"),
    }
  }
}

/// What was read of a leading subscription before the person leaves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Leaving {
  /// What would stop or weaken the work, the most stopping first; empty when it is ready to be left.
  pub weak: Vec<Weak>,
  /// What the app cannot tell from here.
  pub not_checked: Vec<NotChecked>,
}

impl Leaving {
  /// Whether nothing the app can read stands in the way of leaving. Closing the window always stops the work, so
  /// that line alone does not make it unready: it is what the person must not do.
  pub fn ready(&self) -> bool {
    self.weak.iter().all(|weak| *weak == Weak::StopsWithWindow)
  }
}

/// Reads what would stop or weaken what `subscription` leads in the project at `root`, with the app set as `app`.
/// Blocks on SQLite and git.
pub fn leaving_in(
  (root, programs): (&Path, &Programs),
  subscription: &Subscription,
  app: AppSet,
) -> Result<Leaving, Message> {
  let stored = |error: StoreError| messages::store_error(&error);
  let db = ProjectDb::open_read_only(root).map_err(stored)?;
  let mut weak = Vec::new();
  if !app.drives {
    weak.push(Weak::DriveOff);
  }
  if let Some(why) = orchestrator_unready(root)? {
    weak.push(Weak::OrchestratorNotReady(why));
  }
  let led = Reads::read(&db, root)
    .and_then(|reads| reads.led(&db, subscription))
    .map_err(stored)?;
  let unstarted = |named: &[&str]| -> Vec<String> {
    led
      .standing
      .tasks
      .iter()
      .filter(|(_, standing)| named.contains(&standing.name()))
      .map(|(task, _)| task.clone())
      .collect()
  };
  let without = unstarted(&["no_pipeline", "pipeline_not_ready"]);
  if !without.is_empty() {
    weak.push(Weak::NoPipeline(without));
  }
  let (tasks, pipelines) =
    without_deliver_step(&db, &unstarted(&["ready_to_start", "blocked_by", "held"])).map_err(stored)?;
  if !tasks.is_empty() {
    weak.push(Weak::NoDeliverStep { tasks, pipelines });
  }
  if lands_in_checkout(&db, subscription.target()).map_err(stored)? {
    let runner = programs.runner();
    match Workspaces::new(&runner, Path::new(GIT), root).uncommitted(root) {
      Ok(files) if !files.is_empty() => weak.push(Weak::CheckoutHasChanges { files: files.len() }),
      Ok(_) => {}
      Err(error) => tracing::info!(%error, "the checkout's changes could not be read before leaving"),
    }
  }
  if db.project_limits().map_err(stored)?.daily_budget_limit().is_none() {
    weak.push(Weak::NoDailyBudget);
  }
  if !app.keeps_awake {
    weak.push(Weak::MaySleep);
  }
  weak.push(Weak::StopsWithWindow);
  Ok(Leaving {
    weak,
    not_checked: vec![NotChecked::Servers],
  })
}

/// [`leaving_in`] of what conversation `chat` leads to the end as `target`; `None` when it does not lead that
/// target to the end now (it only follows it, or stopped): a conversation shows nothing then. Blocks on SQLite and
/// git.
pub fn leaving_of(
  place: (&Path, &Programs),
  (chat, target): (ChatId, WatchTarget),
  app: AppSet,
) -> Result<Option<Leaving>, Message> {
  let subscriptions = ProjectDb::open_read_only(place.0)
    .and_then(|db| db.active_subscriptions())
    .map_err(|error| messages::store_error(&error))?;
  let leading = subscriptions
    .of_chat(chat)
    .find(|subscription| subscription.target() == target && subscription.mode().ceilings().is_some());
  leading
    .map(|subscription| leaving_in(place, subscription, app))
    .transpose()
}

/// Why the Orchestrator cannot run a turn now; `None` when it can.
fn orchestrator_unready(root: &Path) -> Result<Option<Message>, Message> {
  let readiness = OrchestratorSetup::load_in(root)?.readiness();
  Ok((!readiness.is_ready()).then(|| Message::new("orchestrator.leaving.orchestratorNotReady")))
}

/// Those of `tasks`, by their keys, whose pipeline's latest published version has no Deliver step, and those
/// pipelines, each once, in the order their tasks come.
fn without_deliver_step(db: &ProjectDb, tasks: &[String]) -> Result<(Vec<String>, Vec<PipelineRef>), StoreError> {
  let mut without = Vec::new();
  let mut pipelines: Vec<PipelineRef> = Vec::new();
  for task in recorded::all(db)? {
    let key = task.key().to_string();
    if !tasks.contains(&key) {
      continue;
    }
    let Some(pipeline) = task.details().pipeline else {
      continue;
    };
    let delivers = db
      .latest_version::<Pipeline>(pipeline)?
      .is_some_and(|version| version.body().deliver().is_some());
    if delivers {
      continue;
    }
    without.push(key);
    if pipelines.iter().all(|known| known.id != pipeline)
      && let Some(resource) = db.resource(pipeline)?
    {
      pipelines.push(PipelineRef {
        id: pipeline,
        name: resource.name().as_str().to_owned(),
      });
    }
  }
  Ok((without, pipelines))
}

/// Whether the work of `target` merges into the branch the person's checkout has: a module with neither a branch of
/// its own nor a base branch set, and any target that is no module.
fn lands_in_checkout(db: &ProjectDb, target: WatchTarget) -> Result<bool, StoreError> {
  Ok(match target {
    WatchTarget::Module(id) => db
      .module(id)?
      .is_none_or(|module| module.run_base() == BaseChoice::CheckedOut),
    WatchTarget::Task(_) | WatchTarget::WithoutModule | WatchTarget::Project => true,
  })
}
