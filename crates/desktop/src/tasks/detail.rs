//! A task with its latest run, as the task page (02.10 `RD4pc`, 02.I `SfXU1`) shows it: the
//! run's steps in pipeline order with the agent and model each runs on, and the run's CLI
//! invocations. Read from the project database, which the runner writes; blocks on SQLite, so
//! the UI calls it off the UI thread.

use std::{num::NonZeroU32, path::Path, sync::Arc};

use orchester_core::{
  connection::{AgentProvider, ConnectionKind, invocation::Platform},
  id::{ConnectionId, InvocationId, RunId, StepExecutionId, TaskId},
  permission::AgentPermissions,
  resource::{AgentProfile, Step, StepKey, StepKind, VersionNumber},
  run::{
    ActivityEntry, Invocation, Run, RunEventKind, RunStatus, StepExecutor, StepFailure, StepOutput, StepStatus,
    TaskPhase, TokenUsage, active_time,
  },
  store::{ActivityStore, ConnectionStore, ResourceStore, RunStore, TaskStore},
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
  /// The agent CLI of the profile version's connection, while that connection exists.
  pub provider: Option<AgentProvider>,
  pub model: Option<Arc<str>>,
  /// What the step's agent may do in this run; `None` for a tool step, or when its profile version
  /// is gone.
  pub permissions: Option<RunPermissions>,
  pub execution: Option<StepExecutionId>,
  pub status: Option<StepStatus>,
  /// The attempt of the latest execution.
  pub attempt: Option<NonZeroU32>,
  /// Why the latest execution failed, when it did and its log says.
  pub failure: Option<StepFailure>,
  /// The step's stage in the pipeline graph, from 0: steps of one stage run beside each other.
  pub stage: usize,
}

/// A step's permissions in its run: the pinned profile version's, narrowed by the step, as its
/// connection's CLI runs them on this platform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunPermissions {
  pub permissions: AgentPermissions,
  /// Network off was chosen, but the CLI cannot enforce it, so network stays on.
  pub network_stays_on: bool,
  /// The profile version the permissions come from.
  pub profile: VersionNumber,
}

/// The latest run of a task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunView {
  pub run: Run,
  pub pipeline: Arc<str>,
  pub version: VersionNumber,
  pub steps: Vec<StepView>,
  pub invocations: Vec<Invocation>,
  /// When a paused run paused or stopped at its limit.
  pub paused_at: Option<Timestamp>,
  /// The run's work was committed as a checkpoint right before it paused.
  pub saved_at_pause: bool,
  /// What its step executions left for the steps after them.
  pub outputs: Vec<StepOutput>,
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

  /// Active agent time of the run so far: pauses and waits for a person do not count.
  pub fn active_time(&self, now: Timestamp) -> std::time::Duration {
    active_time(&self.invocations, now)
  }

  /// The step a run stopped at its limit was working on, if it stopped mid-step.
  pub fn stopped_step(&self) -> Option<&StepView> {
    self.steps.iter().find(|step| step.status == Some(StepStatus::Running))
  }

  /// Steps an agent works on now, in pipeline order.
  pub fn working_steps(&self) -> impl Iterator<Item = &StepView> {
    self
      .steps
      .iter()
      .filter(|step| matches!(step.status, Some(StepStatus::Running | StepStatus::Waiting(_))))
  }

  /// The output `execution` left, if any.
  pub fn output(&self, execution: StepExecutionId) -> Option<&StepOutput> {
    self.outputs.iter().find(|output| output.execution == execution)
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

  /// Whether the run is recorded as running while no runner is `online`: its agent is gone and
  /// the run is interrupted until a runner is back ([`TaskPhase::without_runner`]).
  pub fn lost_its_runner(&self, online: bool) -> bool {
    let phase = self.phase();
    !online && phase.without_runner() != phase
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

/// `run` with its steps and invocations, as the task page shows it.
pub fn run_view(db: &ProjectDb, run: Run) -> Result<RunView, StoreError> {
  let pinned = run.snapshot().pipeline();
  let pipeline = db
    .resource(pinned.resource)?
    .map_or_else(|| Arc::from(""), |resource| resource.name().as_str().into());
  let mut steps = Vec::new();
  let stages = run.snapshot().body().stages();
  let stage_of = |key: &StepKey| {
    stages
      .iter()
      .position(|stage| stage.iter().any(|step| step.key == *key))
      .unwrap_or_default()
  };
  for (index, step) in run.snapshot().body().steps().iter().enumerate() {
    let (agent, provider, model, permissions) = match run.snapshot().step(&step.key).map(|pinned| pinned.executor()) {
      Some(StepExecutor::Agent { profile, model, .. }) => {
        let version = db.version::<AgentProfile>(profile.resource, profile.number)?;
        let role = version
          .as_ref()
          .map(|version| Arc::<str>::from(version.body().role().as_str()));
        let provider = match &version {
          Some(version) => provider_of(db, version.body().connection())?,
          None => None,
        };
        let permissions = version.map(|version| run_permissions(step, version.body(), provider, profile.number));
        (role, provider, Some(Arc::<str>::from(model.as_str())), permissions)
      }
      Some(StepExecutor::Tool { .. }) | None => (None, None, None, None),
    };
    let latest = run.latest_execution(&step.key);
    let attempt = latest.and_then(|execution| {
      run
        .attempts()
        .iter()
        .find(|attempt| attempt.execution(execution.id()).is_some())
        .map(orchester_core::run::Attempt::number)
    });
    steps.push(StepView {
      number: index + 1,
      key: step.key.clone(),
      name: step.name.as_str().into(),
      agent,
      provider,
      model,
      permissions,
      execution: latest.map(|execution| execution.id()),
      status: latest.map(|execution| execution.status()),
      attempt,
      failure: latest.and_then(|execution| execution.failure()),
      stage: stage_of(&step.key),
    });
  }
  let (paused_at, saved_at_pause) = match run.status() {
    RunStatus::Paused(_) => {
      let events = db.events(run.id()).map_err(run_store_error)?;
      let paused = events
        .iter()
        .rposition(|event| matches!(event.kind(), RunEventKind::Paused { .. }));
      let saved = paused
        .is_some_and(|index| index > 0 && matches!(events[index - 1].kind(), RunEventKind::CheckpointRecorded { .. }));
      (paused.map(|index| events[index].at()), saved)
    }
    _ => (None, false),
  };
  Ok(RunView {
    invocations: db.invocations(run.id())?,
    outputs: db.step_outputs(run.id())?,
    paused_at,
    saved_at_pause,
    pipeline,
    version: pinned.number,
    steps,
    run,
  })
}

/// `profile`'s permissions narrowed by `step`. Network off that `provider` cannot enforce stays
/// on; permissions it cannot enforce at all never ran (preflight refuses them) and read as chosen.
fn run_permissions(
  step: &Step,
  profile: &AgentProfile,
  provider: Option<AgentProvider>,
  version: VersionNumber,
) -> RunPermissions {
  let permissions = match &step.kind {
    StepKind::Agent {
      repository,
      permissions,
      ..
    } => profile.permissions().narrowed(*repository, permissions),
    StepKind::Tool { .. } => profile.permissions().clone(),
  };
  let network_stays_on = provider
    .and_then(|provider| permissions.enforced_on(provider, Platform::current()).ok())
    .is_some_and(|enforced| enforced.network_stays_on().is_some());
  RunPermissions {
    permissions,
    network_stays_on,
    profile: version,
  }
}

fn provider_of(db: &ProjectDb, connection: ConnectionId) -> Result<Option<AgentProvider>, StoreError> {
  Ok(
    match db.connection(connection)?.map(|connection| connection.kind().clone()) {
      Some(ConnectionKind::Agent(provider)) => Some(provider),
      Some(ConnectionKind::GitHub { .. }) | None => None,
    },
  )
}

/// The transcript of `run` as stored.
pub fn activity_in(repo_root: &Path, run: RunId) -> Result<Vec<ActivityEntry>, StoreError> {
  ProjectDb::open(repo_root)?.activity(run, 0)
}

pub(crate) fn run_store_error(error: orchester_store::RunStoreError) -> StoreError {
  match error {
    orchester_store::RunStoreError::Store(error) => error,
    other => StoreError::Corrupt {
      what: "run",
      detail: other.to_string(),
    },
  }
}
