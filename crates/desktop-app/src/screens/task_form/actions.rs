//! What the form's actions do, off the UI thread. *Create* and *Save changes* save the details as a draft
//! under the form's key or over the task it edits. *Create & run* checks that the run can start, saves
//! and queues the task, then asks the runner to start its first run; *Retry run* asks again for the
//! task already queued. Every step is safe to repeat: the form's key names one task, and a task's first
//! run is started under a key of the task's own, so no retry makes a second task or a second run
//! (`tasks::run`). A shared task is claimed for the person before its run starts, as the deprecated
//! desktop does.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::Signal,
};
use orchester_core::{
  id::{ModuleId, TaskId},
  project::TaskKey,
  resource::VersionNumber,
  source::SourcePath,
  task::{
    Task, TaskDetails, TaskStatus, TaskUpdate,
    estimate::StoryPoints,
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
  sources::task as sources,
  tasks::{
    conflict::{Edit, EditConflict, EditSaved, save_edit},
    data::SaveTarget,
    estimate::{Estimates, apply_person_points},
    run::{QueueFailure, QueuedStart, StartOutcome, queue_in, queue_organized_in, start_messages},
    writer::{TaskWriter, WriteError},
  },
  workspace::OpenProject,
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
  /// The task's organization as the edits were made on it and as the form leaves it: of the organization the form
  /// changes only *Don't merge automatically* (ORC-69); the store puts that change on an organization changed
  /// meanwhile.
  pub organization: (TaskOrganization, TaskOrganization),
  /// The person's story points; `None`: Automatic. Stored beside the task once it is saved.
  pub points: Option<StoryPoints>,
  /// The module the task goes in once its details are saved.
  pub module: Option<ModuleId>,
  /// A new task's sources, each confirmed by the person on the form, stored with it; a stored task's are saved as
  /// they change (`super::sources`).
  pub sources: Vec<SourcePath>,
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

/// Saves the form; once a task is stored, Orchester is asked for its story points (`estimates`, which estimates them
/// only while the person allows it and has set none).
pub fn save_action(ctx: &mut Ctx, servers: Servers, (estimates, open): (Estimates, OpenProject)) -> SaveAction {
  ctx.future_action(move |args: SaveArgs| {
    let (servers, estimates, open) = (servers.clone(), estimates.clone(), open.clone());
    async move {
      let outcome = background::run(move || store(&args, &servers))
        .await
        .map_err(Problems::one)??;
      if let SaveOutcome::Stored(task) = &outcome {
        estimates.request(open, task.id());
      }
      Ok(outcome)
    }
  })
}

fn write_problem(error: &WriteError) -> Problems {
  tracing::error!(%error, "cannot save the task");
  Problems::one(error.message())
}

/// `task`, saved, with the person's `points` stored beside it.
fn with_points(repo_root: &Path, task: Task, points: Option<StoryPoints>) -> Result<Task, Problems> {
  apply_person_points(repo_root, task, points).map_err(|error| {
    tracing::error!(%error, "cannot store the task's story points");
    Problems::one(messages::store_error(&error))
  })
}

/// Saves `args` through the project's task writer: a shared project's server, else its database. A new
/// draft under the form's key; an edit on the version it was made on, merged into a version stored
/// meanwhile when no field both changed, else a conflict for the person.
fn store(args: &SaveArgs, servers: &Servers) -> Result<SaveOutcome, Problems> {
  let mut writer = TaskWriter::open(&args.repo_root, servers).map_err(|error| write_problem(&error))?;
  let organization = (&args.organization.0, &args.organization.1);
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
      sources::save_new_in(&args.repo_root, task.id(), &args.sources, true).map_err(Problems::one)?;
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
  let task = with_points(&args.repo_root, task, args.points)?;
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
  /// A new task's sources that were not stored when it was queued: Retry run stores them before it asks for the run.
  pub unsourced: Vec<SourcePath>,
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
  /// Orchester's story-point estimates, and the project they are asked in.
  pub estimates: (Estimates, OpenProject),
}

pub fn run_action(ctx: &mut Ctx, services: RunServices) -> RunAction {
  ctx.future_action(move |args: RunArgs| {
    let services = services.clone();
    async move {
      let (queued, claim) = match args {
        RunArgs::New { save, version } => queue(&services, save, version).await?,
        RunArgs::Retry(queued) => (store_unsourced(&services, queued).await?, WorkClaim::default()),
      };
      start(&services, queued, claim).await
    }
  })
}

/// Saves and queues the task, with the organization the form changed, and stores a new task's sources before its run
/// is asked to start, which snapshots them; why they were not stored, if they were not. Blocks on SQLite.
fn queue_with_sources(
  save: SaveArgs,
  servers: &Servers,
  version: VersionNumber,
) -> (Result<QueuedStart, QueueFailure>, Option<Message>) {
  let (base, edited) = &save.organization;
  let queued = if base == edited {
    queue_in(&save.repo_root, servers, save.target, save.details, version)
  } else {
    queue_organized_in(
      &save.repo_root,
      servers,
      save.target,
      save.details,
      (base, edited),
      version,
    )
  };
  let unsourced = match (&queued, save.target) {
    (Ok(queued), SaveTarget::New(_)) => {
      sources::save_new_in(&save.repo_root, queued.task.id(), &save.sources, true).err()
    }
    _ => None,
  };
  (queued, unsourced)
}

/// Saves and queues the task ([`queue_with_sources`]); nothing is queued when the run could not start with its
/// configuration.
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
  let (repo_root, target, points, module) = (save.repo_root.clone(), save.target, save.points, save.module);
  let sources = save.sources.clone();
  let (queued, unsourced) = background::run(move || queue_with_sources(*save, &servers, version))
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
    unsourced: Vec::new(),
  };
  services.queued.set(Some(task.clone()));
  if let Some(message) = unsourced {
    // The task is queued: the form keeps it, with Retry run, which stores the sources before it asks for the run,
    // and says why they were not stored.
    return Err(RunProblems {
      messages: Arc::from([message]),
      queued: Some(QueuedTask {
        unsourced: sources,
        ..task
      }),
      saved: None,
    });
  }
  let in_module = queued.task.module() == module;
  keep_points(services, repo_root.clone(), queued.task, points).await;
  if !in_module {
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

/// Stores the person's `points` beside the queued `task` and asks for Orchester's estimate. The run goes on either
/// way: points that were not stored are logged, and the person sets them again on the task.
async fn keep_points(services: &RunServices, repo_root: PathBuf, task: Task, points: Option<StoryPoints>) {
  let id = task.id();
  let stored = background::run(move || with_points(&repo_root, task, points)).await;
  if let Ok(Err(problems)) = stored {
    tracing::warn!(?problems, "the queued task's story points were not stored");
  }
  let (estimates, open) = &services.estimates;
  estimates.request(open.clone(), id);
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

/// Stores the sources a new task was queued without, before Retry run asks for its run, which snapshots them; the
/// task stays queued with them when they still cannot be stored.
async fn store_unsourced(services: &RunServices, queued: QueuedTask) -> Result<QueuedTask, RunProblems> {
  if queued.unsourced.is_empty() {
    return Ok(queued);
  }
  let (root, task, unsourced) = (services.repo_root.clone(), queued.task, queued.unsourced.clone());
  let stored = background::run(move || sources::save_new_in(&root, task, &unsourced, true)).await;
  match stored.and_then(|stored| stored) {
    Ok(()) => Ok(QueuedTask {
      unsourced: Vec::new(),
      ..queued
    }),
    Err(message) => Err(RunProblems {
      messages: Arc::from([message]),
      queued: Some(queued),
      saved: None,
    }),
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
