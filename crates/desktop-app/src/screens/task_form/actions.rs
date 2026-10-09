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
  id::TaskId,
  project::TaskKey,
  resource::VersionNumber,
  task::{Task, TaskDetails, TaskUpdate, organization::TaskOrganization},
  time::Timestamp,
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  runner::Runners,
  sharing::{Servers, project::WorkClaim},
  tasks::{
    data::SaveTarget,
    run::{StartOutcome, queue_in, start_messages},
    writer::TaskWriter,
  },
};

/// What a save stores, and where.
#[derive(Clone)]
pub struct SaveArgs {
  pub repo_root: PathBuf,
  pub target: SaveTarget,
  pub details: TaskDetails,
  /// The task's organization, unchanged: the form does not edit it yet.
  pub organization: TaskOrganization,
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

pub type SaveAction = FutureAction<SaveArgs, Saved, Problems>;

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

/// Saves `args` through the project's task writer: a shared project's server, else its database.
fn store(args: &SaveArgs, servers: &Servers) -> Result<Saved, Problems> {
  let write = |error: orchester_workspace::tasks::writer::WriteError| {
    tracing::error!(%error, "cannot save the task");
    Problems::one(error.message())
  };
  let mut writer = TaskWriter::open(&args.repo_root, servers).map_err(write)?;
  let organization = (&args.organization, &args.organization);
  match writer
    .save_organized(args.target, args.details.clone(), organization, Timestamp::now())
    .map_err(write)?
  {
    TaskUpdate::Saved(task) => Ok(Saved(Arc::new(task))),
    TaskUpdate::Conflict { .. } => Err(Problems::one(messages::task_changed_meanwhile())),
  }
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

/// The task whose run started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Started(pub TaskKey);

impl lurq::app::component::DevtoolsInspectable for Started {}

/// Why the run did not start, and the task queued for it, if it was queued.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunProblems {
  pub messages: Arc<[Message]>,
  pub queued: Option<QueuedTask>,
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
  let not_queued = |messages: Arc<[Message]>| RunProblems { messages, queued: None };
  let servers = services.servers.clone();
  let queued = background::run(move || queue_in(&save.repo_root, &servers, save.target, save.details, version))
    .await
    .map_err(|message| not_queued(Arc::from([message])))?
    .map_err(|failure| {
      tracing::warn!(?failure, "the task was not queued for a run");
      not_queued(failure.messages().into())
    })?;
  let task = QueuedTask {
    task: queued.task.id(),
    key: queued.task.key().clone(),
    outcome: StartOutcome::ConfirmedNotStarted,
  };
  services.queued.set(Some(task.clone()));
  Ok((task, queued.claim))
}

/// Asks the runner to start the queued task's first run under the task's own key.
async fn start(services: &RunServices, queued: QueuedTask, claim: WorkClaim) -> Result<Started, RunProblems> {
  let refused = |messages: Arc<[Message]>, queued: QueuedTask| RunProblems {
    messages,
    queued: Some(queued),
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
    Ok(_) => Ok(Started(queued.key)),
    Err(error) => {
      tracing::warn!(%error, "the start request failed");
      claim.release(&services.servers).await;
      let outcome = queued.outcome.after(&error);
      Err(refused(start_messages(&error).into(), QueuedTask { outcome, ..queued }))
    }
  }
}
