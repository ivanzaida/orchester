//! What the form's actions do, off the UI thread. *Create* and *Save changes* save the details as a draft
//! under the form's key or over the task it edits. *Create & run* checks that the run can start, saves
//! and queues the task, then asks the runner to start its first run; *Retry run* asks again for the
//! task already queued. Every step is safe to repeat: the form's key names one task, and a task's first
//! run is started under a key of the task's own, so no retry makes a second task or a second run
//! (`tasks::run`). A shared task is claimed for the person before its run starts, as the deprecated
//! desktop does.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::Signal,
};
use orchester_core::{
  id::{ModuleId, TaskId},
  project::TaskKey,
  resource::VersionNumber,
  task::{
    Task, TaskDetails, TaskStatus, TaskUpdate,
    organization::{ChangeActor, TaskOrganization},
  },
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;
use orchester_workspace::{
  background,
  messages::{self, Message},
  runner::{RunRequestError, Runners},
  sharing::{Servers, project::WorkClaim},
  tasks::{
    conflict::{Edit, EditConflict, EditSaved, save_edit},
    data::SaveTarget,
    run::{StartOutcome, queue_in, start_messages},
    writer::{TaskWriter, WriteError},
  },
};

/// What a save stores, and where.
#[derive(Clone)]
pub struct SaveArgs {
  pub repo_root: PathBuf,
  pub target: SaveTarget,
  pub details: TaskDetails,
  /// The details of the version the edits were made on, for an edit: what the person changed is put
  /// on a version stored meanwhile (`tasks::conflict::save_edit`).
  pub base: Option<TaskDetails>,
  /// The task's organization, unchanged: the form does not edit it yet.
  pub organization: TaskOrganization,
  /// The module the task goes in once its details are saved.
  pub module: Option<ModuleId>,
}

/// The task a save stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved(pub Arc<Task>);

impl lurq::app::component::DevtoolsInspectable for Saved {}

/// Why an action did not do what it was asked, in the workspace's words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problems(pub Arc<[Message]>);

impl lurq::app::component::DevtoolsInspectable for Problems {}

impl Problems {
  fn one(message: Message) -> Self {
    Self(Arc::from([message]))
  }
}

/// How a save ended: the task stored, or a newer version whose fields both changed, for the person to
/// choose from (`Edit task · conflict` `PenUu`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveOutcome {
  Stored(Arc<Task>),
  Conflict(Arc<EditConflict>),
}

impl lurq::app::component::DevtoolsInspectable for SaveOutcome {}

pub type SaveAction = FutureAction<SaveArgs, SaveOutcome, Problems>;

pub fn save_action(ctx: &mut Ctx, servers: Servers) -> SaveAction {
  ctx.future_action(move |args: SaveArgs| {
    let servers = servers.clone();
    async move {
      background::run(move || store(&args, &servers))
        .await
        .map_err(Problems::one)?
    }
  })
}

fn write_problem(error: &WriteError) -> Problems {
  tracing::error!(%error, "cannot save the task");
  Problems::one(error.message())
}

/// Saves `args` through the project's task writer: a shared project's server, else its database. A new
/// draft under the form's key; an edit on the version it was made on, merged into a version stored
/// meanwhile when no field both changed, else a conflict for the person.
fn store(args: &SaveArgs, servers: &Servers) -> Result<SaveOutcome, Problems> {
  let mut writer = TaskWriter::open(&args.repo_root, servers).map_err(|error| write_problem(&error))?;
  let organization = (&args.organization, &args.organization);
  let (task, details) = match args.target {
    SaveTarget::New(_) => {
      let task = match writer
        .save_organized(args.target, args.details.clone(), organization, Timestamp::now())
        .map_err(|error| write_problem(&error))?
      {
        TaskUpdate::Saved(task) => task,
        TaskUpdate::Conflict { .. } => return Err(Problems::one(messages::task_changed_meanwhile())),
      };
      // A new draft's key may name a task an earlier attempt queued: it is not a draft to save over.
      if task.status() != TaskStatus::Draft {
        return Err(Problems::one(messages::task_no_longer_draft()));
      }
      (task, args.details.clone())
    }
    SaveTarget::Task(task, read) => {
      let edit = Edit {
        task,
        read,
        // Handlers always name the version; without one nothing of a newer version could be kept.
        base: args.base.as_ref().unwrap_or(&args.details),
        details: args.details.clone(),
        organization,
      };
      match save_edit(&mut writer, edit, Timestamp::now()).map_err(|error| write_problem(&error))? {
        EditSaved::Saved { task, details } => (task, details),
        EditSaved::Conflict(conflict) => return Ok(SaveOutcome::Conflict(Arc::new(conflict))),
      }
    }
  };
  if task.details() != &details {
    tracing::error!(task = %task.key(), "the stored details differ from the ones saved");
    return Err(Problems::one(messages::task_saved_differently()));
  }
  let task = moved(&mut writer, task, args.module).map_err(|error| write_problem(&error))?;
  Ok(SaveOutcome::Stored(Arc::new(task)))
}

/// `task` in `module`, or in none, when it is not there already.
fn moved(writer: &mut TaskWriter, task: Task, module: Option<ModuleId>) -> Result<Task, WriteError> {
  if task.module() == module {
    return Ok(task);
  }
  writer.set_task_module(task.id(), module, ChangeActor::Person)?;
  Ok(task.with_module(module))
}

/// A task queued for its first run, which has not started (yet).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuedTask {
  pub task: TaskId,
  pub key: TaskKey,
  /// Whether a failed start certainly made no run.
  pub outcome: StartOutcome,
}

impl lurq::app::component::DevtoolsInspectable for QueuedTask {}

/// Create & run with the published `version` of the chosen pipeline, or Retry run of a queued task.
#[derive(Clone)]
pub enum RunArgs {
  New {
    save: Box<SaveArgs>,
    version: VersionNumber,
  },
  Retry(QueuedTask),
}

/// The task whose run was asked to start, and whether it did: one that failed before it started opens on the Summary,
/// which says why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Started(pub TaskKey, pub RunBegan);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunBegan {
  Yes,
  FailedBeforeStart,
}

impl lurq::app::component::DevtoolsInspectable for Started {}

/// Why the run did not start, and the task queued for it, if it was queued, or else the draft Create &
/// run saved before it stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunProblems {
  pub messages: Arc<[Message]>,
  pub queued: Option<QueuedTask>,
  pub saved: Option<Saved>,
}

impl lurq::app::component::DevtoolsInspectable for RunProblems {}

pub type RunAction = FutureAction<RunArgs, Started, RunProblems>;

/// The services a run request uses.
#[derive(Clone)]
pub struct RunServices {
  pub runners: Runners,
  pub servers: Servers,
  pub repo_root: PathBuf,
  /// Set as soon as the task is queued, so the form shows it while the start request is under way.
  pub queued: Signal<Option<QueuedTask>>,
}

pub fn run_action(ctx: &mut Ctx, services: RunServices) -> RunAction {
  ctx.future_action(move |args: RunArgs| {
    let services = services.clone();
    async move {
      let (queued, claim) = match args {
        RunArgs::New { save, version } => queue(&services, save, version).await?,
        RunArgs::Retry(queued) => (queued, WorkClaim::default()),
      };
      start(&services, queued, claim).await
    }
  })
}

/// Saves and queues the task; nothing is queued when the run could not start with its configuration.
async fn queue(
  services: &RunServices,
  save: Box<SaveArgs>,
  version: VersionNumber,
) -> Result<(QueuedTask, WorkClaim), RunProblems> {
  let not_queued = |messages: Arc<[Message]>| RunProblems {
    messages,
    queued: None,
    saved: None,
  };
  let servers = services.servers.clone();
  let (repo_root, target, module) = (save.repo_root.clone(), save.target, save.module);
  let queued = background::run(move || queue_in(&save.repo_root, &servers, save.target, save.details, version))
    .await
    .map_err(|message| not_queued(Arc::from([message])))?;
  let queued = match queued {
    Ok(queued) => queued,
    Err(failure) => {
      tracing::warn!(?failure, "the task was not queued for a run");
      let saved = saved_draft(repo_root, target).await;
      return Err(RunProblems {
        saved,
        ..not_queued(failure.messages().into())
      });
    }
  };
  let task = QueuedTask {
    task: queued.task.id(),
    key: queued.task.key().clone(),
    outcome: StartOutcome::ConfirmedNotStarted,
  };
  services.queued.set(Some(task.clone()));
  if queued.task.module() != module {
    let (servers, id) = (services.servers.clone(), task.task);
    let moved = background::run(move || {
      let mut writer = TaskWriter::open(&repo_root, &servers)?;
      writer.set_task_module(id, module, ChangeActor::Person)
    })
    .await;
    // The task is queued: the form keeps it, with Retry run, and says why it is not in the module.
    let refused = |message: Message| RunProblems {
      messages: Arc::from([message]),
      queued: Some(task.clone()),
      saved: None,
    };
    match moved {
      Ok(Ok(_)) => {}
      Ok(Err(error)) => {
        tracing::warn!(%error, "the queued task was not put in its module");
        return Err(refused(error.message()));
      }
      Err(message) => return Err(refused(message)),
    }
  }
  Ok((task, queued.claim))
}

/// The draft a Create & run that stopped before queuing saved under the form's key, so the form goes on
/// editing it rather than making a second task. Nothing for a form that edits a task already.
async fn saved_draft(repo_root: PathBuf, target: SaveTarget) -> Option<Saved> {
  let SaveTarget::New(batch) = target else {
    return None;
  };
  let found = background::run(move || ProjectDb::open(&repo_root).and_then(|db| db.task_by_batch(batch))).await;
  match found {
    Ok(Ok(task)) => task
      .filter(|task| task.status() == TaskStatus::Draft)
      .map(|task| Saved(Arc::new(task))),
    Ok(Err(error)) => {
      tracing::warn!(%error, "cannot tell whether Create & run saved its task");
      None
    }
    // Background work that did not finish is logged by `background::run`.
    Err(_) => None,
  }
}

/// Asks the runner to start the queued task's first run under the task's own key.
async fn start(services: &RunServices, queued: QueuedTask, claim: WorkClaim) -> Result<Started, RunProblems> {
  let refused = |messages: Arc<[Message]>, queued: QueuedTask| RunProblems {
    messages,
    queued: Some(queued),
    saved: None,
  };
  let claim = claim.and(
    services
      .servers
      .claim_for_work(&services.repo_root, queued.task)
      .await
      .map_err(|refusal| refused(Arc::from([refusal.message()]), queued.clone()))?,
  );
  match services
    .runners
    .start_run(queued.task.first_run_key(), queued.task)
    .await
  {
    Ok(_) => Ok(Started(queued.key, RunBegan::Yes)),
    // The run failed before it started: the task's page says why and runs it again, which retrying here cannot.
    Err(RunRequestError::Rejected(Rejection::RunNotStarted { .. })) => {
      claim.release(&services.servers).await;
      Ok(Started(queued.key, RunBegan::FailedBeforeStart))
    }
    Err(error) => {
      tracing::warn!(%error, "the start request failed");
      claim.release(&services.servers).await;
      let outcome = queued.outcome.after(&error);
      Err(refused(start_messages(&error).into(), QueuedTask { outcome, ..queued }))
    }
  }
}
