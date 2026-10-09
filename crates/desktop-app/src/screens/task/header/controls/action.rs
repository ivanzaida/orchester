//! What a run control asks of the page's project: run a ready draft (queue it, then start its first
//! run), start a queued task's first run, pause, resume, continue or cancel its run. Each goes to the
//! runner of the project the page shows, whichever tab is active. Work on a shared task goes on only
//! under the person's claim; Pause and Cancel need none.

use lurq::app::ctx::{Ctx, FutureAction};
use orchester_core::{
  id::{RunId, TaskId},
  store::TaskStore,
  task::{MissingDetail, QueueError},
};
use orchester_ipc::Command;
use orchester_store::{ProjectDb, StoreError};
use orchester_workspace::{
  background,
  messages::{self, Message},
  runner::Runners,
  sharing::{Servers, project::WorkClaim},
  tasks::{
    cancel_run::{Stop, stop_named_run_in},
    data::{SaveTarget, published_pipelines},
    run::{QueueFailure, control_message, queue_in},
  },
  workspace::OpenProject,
};

/// A request about the task's run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunControl {
  /// Queue a draft that has every detail and start its first run.
  Run(TaskId),
  /// Start the task's first run, under its first-run key: never a second run.
  Start(TaskId),
  Pause(RunId),
  Resume(RunId),
  /// Continue an interrupted run from its last checkpoint.
  Continue(RunId),
  /// Cancel the run the page shows, and no run that started since.
  Cancel(RunId),
}

impl lurq::app::component::DevtoolsInspectable for RunControl {}

impl RunControl {
  /// The key of what the frame's notice says when the runner did not carry it out.
  pub const fn refused_key(self) -> &'static str {
    match self {
      Self::Run(_) => "task.controls.refused.run",
      Self::Start(_) => "task.controls.refused.start",
      Self::Pause(_) => "task.controls.refused.pause",
      Self::Resume(_) => "task.controls.refused.resume",
      Self::Continue(_) => "task.controls.refused.continue",
      Self::Cancel(_) => "task.controls.refused.cancel",
    }
  }
}

pub type ControlAction = FutureAction<RunControl, RunControl, Message>;

/// Asks the runner of `project` to carry out a control of `task`'s run.
pub fn control_action(
  ctx: &mut Ctx,
  (project, task): (OpenProject, TaskId),
  (runners, servers): (Runners, Servers),
) -> ControlAction {
  ctx.future_action(move |control: RunControl| {
    let (project, runners, servers) = (project.clone(), runners.clone(), servers.clone());
    async move {
      let claim = match control {
        RunControl::Run(_) => queue(&project, &servers, task).await?,
        RunControl::Start(_) | RunControl::Resume(_) | RunControl::Continue(_) => servers
          .claim_for_work(&project.repo_root, task)
          .await
          .map_err(|refusal| refusal.message())?,
        RunControl::Pause(_) | RunControl::Cancel(_) => WorkClaim::default(),
      };
      let result = carry_out(&project, (&runners, task), control).await;
      if result.is_err() {
        claim.release(&servers).await;
      }
      result.map(|()| control)
    }
  })
}

/// Queues draft `task` on the latest published version of its pipeline, the claim of a shared task
/// taken; the runner checks the run as New task's Run does.
async fn queue(project: &OpenProject, servers: &Servers, task: TaskId) -> Result<WorkClaim, Message> {
  let (root, servers) = (project.repo_root.clone(), servers.clone());
  background::run(move || {
    let (details, revision, version) = {
      let db = ProjectDb::open(&root).map_err(|error| messages::store_error(&error))?;
      let draft = db
        .task(task)
        .and_then(|found| found.ok_or(StoreError::TaskNotFound(task)))
        .map_err(|error| messages::store_error(&error))?;
      let published = published_pipelines(&db).map_err(|error| messages::store_error(&error))?;
      let version = published
        .into_iter()
        .find(|pipeline| Some(pipeline.id) == draft.details().pipeline)
        .map(|pipeline| pipeline.version);
      (draft.details().clone(), draft.revision(), version)
    };
    let Some(version) = version else {
      let unpublished = QueueFailure::Queue(QueueError::Incomplete(vec![MissingDetail::Pipeline]));
      return Err(first(&unpublished));
    };
    queue_in(&root, &servers, SaveTarget::Task(task, revision), details, version)
      .map(|queued| queued.claim)
      .map_err(|failure| first(&failure))
  })
  .await?
}

/// What a person reads first of why the task was not queued.
fn first(failure: &QueueFailure) -> Message {
  failure
    .messages()
    .into_iter()
    .next()
    .unwrap_or_else(messages::interrupted)
}

async fn carry_out(
  project: &OpenProject,
  (runners, task): (&Runners, TaskId),
  control: RunControl,
) -> Result<(), Message> {
  let result = match control {
    RunControl::Run(task) | RunControl::Start(task) => runners
      .start_run_in(project, task.first_run_key(), task)
      .await
      .map(|_| ()),
    RunControl::Pause(run) => runners.control_run_in(project, Command::PauseRun { run }).await,
    RunControl::Resume(run) => runners.control_run_in(project, Command::ResumeRun { run }).await,
    RunControl::Continue(run) => runners.continue_run_in(project, run).await,
    RunControl::Cancel(run) => {
      return stop_named_run_in(project, task, Stop::Confirmed(Some(run)), runners)
        .await
        .map_err(|refused| refused.message());
    }
  };
  result.map_err(|error| {
    tracing::warn!(%error, ?control, "the runner did not carry out the request");
    control_message(&error)
  })
}
