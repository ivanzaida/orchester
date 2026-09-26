//! A task with its latest run, as the task page (02.10 `RD4pc`, 02.I `SfXU1`) shows it: the
//! run's steps in pipeline order with the agent and model each runs on, and the run's CLI
//! invocations. Read from the project database, which the runner writes; blocks on SQLite, so
//! the UI calls it off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId, TaskId},
  resource::{AgentProfile, RepositoryAccess, StepKey, StepKind, VersionNumber},
  run::{ActivityEntry, Invocation, Run, StepExecutor, StepStatus, TaskPhase, TokenUsage},
  store::{ActivityStore, ResourceStore, RunStore, TaskStore},
  task::Task,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

/// One step of the run's pipeline and its latest execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepView {
  /// Position in the pipeline, from 1.
  pub number: usize,
  pub key: StepKey,
  pub name: Arc<str>,
  /// The agent role of the step's profile version; `None` for a tool step.
  pub agent: Option<Arc<str>>,
  pub model: Option<Arc<str>>,
  pub access: Option<RepositoryAccess>,
  pub execution: Option<StepExecutionId>,
  pub status: Option<StepStatus>,
}

/// The latest run of a task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunView {
  pub run: Run,
  pub pipeline: Arc<str>,
  pub version: VersionNumber,
  pub steps: Vec<StepView>,
  pub invocations: Vec<Invocation>,
}

impl RunView {
  pub fn id(&self) -> RunId {
    self.run.id()
  }

  /// The step that is running, else the last one that ran.
  pub fn current_step(&self) -> Option<&StepView> {
    self
      .steps
      .iter()
      .find(|step| matches!(step.status, Some(StepStatus::Running | StepStatus::Waiting(_))))
      .or_else(|| {
        self
          .steps
          .iter()
          .rev()
          .find(|step| step.status.is_some_and(|status| status != StepStatus::Pending))
      })
  }

  /// The step `execution` (of any attempt) executed.
  pub fn step_of(&self, execution: StepExecutionId) -> Option<&StepView> {
    let key = self.run.execution(execution)?.step();
    self.steps.iter().find(|step| step.key == *key)
  }

  pub fn invocation(&self, id: InvocationId) -> Option<&Invocation> {
    self.invocations.iter().find(|invocation| invocation.id == id)
  }

  /// Tokens the CLIs reported for the whole run.
  pub fn usage(&self) -> TokenUsage {
    self.invocations.iter().map(|invocation| invocation.usage).sum()
  }

  /// Active agent time of the run so far, in milliseconds.
  pub fn active_millis(&self, now: Timestamp) -> i64 {
    self
      .invocations
      .iter()
      .map(|invocation| invocation.active_millis(now))
      .sum()
  }

  /// Invocations of `execution`, in launch order.
  pub fn invocations_of(&self, execution: StepExecutionId) -> impl Iterator<Item = &Invocation> {
    self
      .invocations
      .iter()
      .filter(move |invocation| invocation.execution == execution)
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskDetail {
  pub task: Task,
  pub run: Option<RunView>,
}

impl lurq::app::component::DevtoolsInspectable for TaskDetail {}

impl TaskDetail {
  pub fn phase(&self) -> TaskPhase {
    TaskPhase::of(&self.task, self.run.as_ref().map(|run| &run.run))
  }

  pub fn load_in(repo_root: &Path, task: TaskId) -> Result<Self, StoreError> {
    let db = ProjectDb::open(repo_root)?;
    let task = db.task(task)?.ok_or(StoreError::TaskNotFound(task))?;
    let latest = db
      .runs_of_task(task.id())
      .map_err(run_store_error)?
      .into_iter()
      .next_back();
    let run = latest.map(|run| run_view(&db, run)).transpose()?;
    Ok(Self { task, run })
  }
}

fn run_view(db: &ProjectDb, run: Run) -> Result<RunView, StoreError> {
  let pinned = run.snapshot().pipeline();
  let pipeline = db
    .resource(pinned.resource)?
    .map_or_else(|| Arc::from(""), |resource| resource.name().as_str().into());
  let mut steps = Vec::new();
  for (index, step) in run.snapshot().body().steps().iter().enumerate() {
    let (agent, model) = match run.snapshot().step(&step.key).map(|pinned| pinned.executor()) {
      Some(StepExecutor::Agent { profile, model, .. }) => {
        let role = db
          .version::<AgentProfile>(profile.resource, profile.number)?
          .map(|version| Arc::<str>::from(version.body().role().as_str()));
        (role, Some(Arc::<str>::from(model.as_str())))
      }
      Some(StepExecutor::Tool { .. }) | None => (None, None),
    };
    let latest = run.latest_execution(&step.key);
    steps.push(StepView {
      number: index + 1,
      key: step.key.clone(),
      name: step.name.as_str().into(),
      agent,
      model,
      access: match step.kind {
        StepKind::Agent { repository, .. } => Some(repository),
        StepKind::Tool { .. } => None,
      },
      execution: latest.map(|execution| execution.id()),
      status: latest.map(|execution| execution.status()),
    });
  }
  Ok(RunView {
    invocations: db.invocations(run.id())?,
    pipeline,
    version: pinned.number,
    steps,
    run,
  })
}

/// The transcript of `run` as stored.
pub fn activity_in(repo_root: &Path, run: RunId) -> Result<Vec<ActivityEntry>, StoreError> {
  ProjectDb::open(repo_root)?.activity(run, 0)
}

fn run_store_error(error: orchester_store::RunStoreError) -> StoreError {
  match error {
    orchester_store::RunStoreError::Store(error) => error,
    other => StoreError::Corrupt {
      what: "run",
      detail: other.to_string(),
    },
  }
}
