//! A task with its latest run, as the task page (02.10 `RD4pc`, 02.I `SfXU1`) shows it: the
//! run's steps in pipeline order with the agent and model each runs on, and the run's CLI
//! invocations. Read from the project database, which the runner writes; blocks on SQLite, so
//! the UI calls it off the UI thread.

use std::{num::NonZeroU32, path::Path, sync::Arc};

use orchester_core::{
  connection::{AgentConnection, AgentProvider, EngineVendor},
  conversation::Message,
  id::{ConnectionId, InvocationId, RunId, StepExecutionId, TaskId},
  integration::RunMerge,
  resource::{AgentProfile, StepKey, VersionNumber},
  run::{
    ActivityEntry, AttemptStart, Invocation, PauseReason, PinnedVersion, Plan, Run, RunEvent, RunEventKind, RunLimit,
    RunStatus, StepExecutor, StepFailure, StepOutput, StepStatus, TaskPhase, TokenUsage, active_time, attempt_starts,
    execution_tokens,
  },
  store::{ActivityStore, ConnectionStore, ConversationStore, MergeStore, ResourceStore, RunStore, TaskStore},
  task::Task,
  task_group::GroupHold,
  time::Timestamp,
  tokens::{TokenBudget, TokensUsed},
};
use orchester_store::{ProjectDb, StoreError};

pub use super::permissions::{ChangeState, RunPermissions};
use super::{
  index_use::IndexUses,
  permissions::run_permissions,
  unreadable::{EarlierRun, Unreadable},
};

/// One step of the run's pipeline and its latest execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepView {
  /// Position in the pipeline, from 1.
  pub number: usize,
  pub key: StepKey,
  pub name: Arc<str>,
  /// Who executes the step: the agent role of its profile version, or the name of its tool; `None`
  /// once that version or tool is gone.
  pub agent: Option<Arc<str>>,
  /// A tool step: its command tool runs instead of an agent.
  pub tool_step: bool,
  /// The agent CLI of the profile version's connection, while that connection exists.
  pub provider: Option<AgentProvider>,
  /// The base URL of that connection's model API, as the connection is now, when it is a native one:
  /// an invocation does not record the endpoint it reached.
  pub endpoint: Option<Arc<str>>,
  /// Who receives the agent's prompts, as its engine is named (core's
  /// [`orchester_core::connection::AgentConnection::vendor`]), by that connection as it is now.
  pub vendor: Option<EngineVendor>,
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

/// The latest run of a task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunView {
  /// Only the saved original first run can be retried with the task first-run key.
  pub first_run: bool,
  pub group_hold: Option<GroupHold>,
  pub run: Run,
  pub pipeline: Arc<str>,
  pub version: VersionNumber,
  pub steps: Vec<StepView>,
  pub invocations: Vec<Invocation>,
  /// When a paused run paused or stopped at its limit.
  pub paused_at: Option<Timestamp>,
  /// The run's work was committed as a checkpoint right before it paused.
  pub saved_at_pause: bool,
  /// When a finished run (succeeded, failed or cancelled) ended.
  pub finished_at: Option<Timestamp>,
  /// What its step executions left for the steps after them.
  pub outputs: Vec<StepOutput>,
  /// The plans of its gated steps, by step and version.
  pub plans: Vec<Plan>,
  /// What its agent launches got of the code index and used before their first edit.
  pub index: IndexUses,
  /// When each of its attempts started, and whether a person's decision started it; empty for a
  /// run of one attempt.
  pub attempt_starts: Vec<AttemptStart>,
}

impl RunView {
  pub fn id(&self) -> RunId {
    self.run.id()
  }

  /// The step that is running, else the one that failed, else the last one that ran.
  pub fn current_step(&self) -> Option<&StepView> {
    self
      .steps
      .iter()
      .find(|step| matches!(step.status, Some(StepStatus::Running | StepStatus::Waiting(_))))
      // A run that failed stopped at the step that failed, not at one its failure skipped (a step
      // running beside it that started later).
      .or_else(|| self.steps.iter().find(|step| step.status == Some(StepStatus::Failed)))
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

  /// The step a run stopped at its limit was working on, if it stopped mid-step: after a step's
  /// budget, that step.
  pub fn stopped_step(&self) -> Option<&StepView> {
    if let Some(execution) = self.stopped_execution() {
      return self.step_of(execution);
    }
    self.steps.iter().find(|step| step.status == Some(StepStatus::Running))
  }

  /// The execution whose step budget the run stopped at, if it did.
  pub fn stopped_execution(&self) -> Option<StepExecutionId> {
    match self.run.status() {
      RunStatus::Paused(PauseReason::LimitReached(RunLimit::StepBudget { execution })) => Some(*execution),
      _ => None,
    }
  }

  /// What `execution` used of its step's budget in force for the run, over all its invocations;
  /// `None` when its step has no budget.
  pub fn step_budget_use(&self, execution: StepExecutionId) -> Option<(TokensUsed, TokenBudget)> {
    let budget = self.run.step_budget(self.run.execution(execution)?.step())?;
    Some((execution_tokens(&self.invocations, execution), budget))
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

  /// What `execution` left for the steps after it: a gated step's latest plan, else its output.
  pub fn reported(&self, execution: StepExecutionId) -> Option<&str> {
    Plan::output_of(execution, &self.outputs, &self.plans)
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
  /// The task details pinned by the latest readable run, for run-specific review UI.
  pub run_task: Option<Task>,
  /// The latest run, when it can be read.
  pub run: Option<RunView>,
  /// The merge of the latest run into its base branch, once it was merged.
  pub merge: Option<RunMerge>,
  /// The task's conversation, oldest first.
  pub messages: Vec<Message>,
  /// The latest run, when it cannot be read (02.R `SUuwV`): `run` is `None`, and the page lists
  /// the task's other runs instead.
  pub latest_unreadable: Option<Unreadable>,
  /// The task's other runs, newest first, while its latest run cannot be read.
  pub earlier: Vec<EarlierRun>,
  /// The task's earlier runs that cannot be read, oldest first, while its latest run can be.
  pub earlier_unreadable: Vec<Unreadable>,
  /// The keys of the linked blocking tasks that have not finished: while there are any, the
  /// task's run neither starts nor goes on.
  pub blocked_by: Vec<String>,
}

impl lurq::app::component::DevtoolsInspectable for TaskDetail {}

impl TaskDetail {
  /// Where the task stands, from its latest run; as for a task without runs while that run cannot
  /// be read.
  pub fn phase(&self) -> TaskPhase {
    TaskPhase::of(&self.task, self.run.as_ref().map(|run| &run.run), self.merge.as_ref())
  }

  /// Whether the task can be edited: a draft or a task with a recorded run.
  pub fn editable(&self) -> bool {
    let has_run = self.run.is_some() || self.latest_unreadable.is_some();
    self.task.check_editable(has_run).is_ok()
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
    let blocked_by = blocking_keys(&db, &task)?;
    let mut runs = db.runs_of_task(task.id()).map_err(run_store_error)?;
    let messages = db.messages(task.id())?;
    let latest = runs.pop();
    if let Some(Err(latest)) = &latest {
      let earlier = runs
        .iter()
        .rev()
        .map(|run| EarlierRun::read(&db, &task, run))
        .collect::<Result<_, _>>()?;
      return Ok(Self {
        latest_unreadable: Some(Unreadable::of(latest)),
        earlier,
        blocked_by,
        ..Self::without_runs(task, messages)
      });
    }
    let earlier_unreadable = runs
      .iter()
      .filter_map(|run| run.as_ref().err())
      .map(Unreadable::of)
      .collect();
    let latest = latest.and_then(Result::ok);
    let run_task = latest
      .as_ref()
      .map(|run| db.task_for_run(task.id(), run.id()))
      .transpose()?
      .flatten();
    let merge = match &latest {
      Some(run) => db.run_merge(run.id())?,
      None => None,
    };
    Ok(Self {
      run: latest.map(|run| run_view(&db, run)).transpose()?,
      run_task,
      merge,
      earlier_unreadable,
      blocked_by,
      ..Self::without_runs(task, messages)
    })
  }

  fn without_runs(task: Task, messages: Vec<Message>) -> Self {
    Self {
      task,
      run_task: None,
      run: None,
      merge: None,
      messages,
      latest_unreadable: None,
      earlier: Vec::new(),
      earlier_unreadable: Vec::new(),
      blocked_by: Vec::new(),
    }
  }
}

/// The keys of `task`'s unfinished blocking tasks.
fn blocking_keys(db: &ProjectDb, task: &Task) -> Result<Vec<String>, StoreError> {
  db.unfinished_blocking_tasks(task.id())?
    .into_iter()
    .map(|id| {
      let blocker = db.task(id)?.ok_or(StoreError::TaskNotFound(id))?;
      Ok(blocker.key().to_string())
    })
    .collect()
}

/// `run` with its steps and invocations, as the task page shows it.
pub fn run_view(db: &ProjectDb, run: Run) -> Result<RunView, StoreError> {
  let pinned = run.snapshot().pipeline();
  let (first_run, group_hold) = first_run_hold(db, &run)?;
  let pipeline = db
    .resource(pinned.resource)?
    .map_or_else(|| Arc::from(""), |resource| resource.name().as_str().into());
  let invocations = db.invocations(run.id())?;
  let mut steps = Vec::new();
  let stages = run.snapshot().body().stages();
  let stage_of = |key: &StepKey| {
    stages
      .iter()
      .position(|stage| stage.iter().any(|step| step.key == *key))
      .unwrap_or_default()
  };
  for (index, step) in run.snapshot().body().steps().iter().enumerate() {
    let executor = run.snapshot().step(&step.key).map(|pinned| pinned.executor());
    let (agent, reached, model, permissions) = executed(db, (&run, &invocations), &step.key, executor)?;
    let Reached {
      provider,
      endpoint,
      vendor,
    } = reached;
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
      tool_step: matches!(executor, Some(StepExecutor::Tool { .. })),
      provider,
      endpoint,
      vendor,
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
  let finished_at = finished_at(db, &run)?;
  let attempt_starts = starts_of(db, &run)?;
  Ok(RunView {
    attempt_starts,
    first_run,
    group_hold,
    index: IndexUses::read(db, &invocations)?,
    invocations,
    outputs: db.step_outputs(run.id())?,
    plans: db.plans(run.id()).map_err(run_store_error)?,
    paused_at,
    saved_at_pause,
    finished_at,
    pipeline,
    version: pinned.number,
    steps,
    run,
  })
}

/// When each attempt of `run` started and who started it; nothing to read for a run of one attempt.
fn starts_of(db: &ProjectDb, run: &Run) -> Result<Vec<AttemptStart>, StoreError> {
  if run.attempts().len() < 2 {
    return Ok(Vec::new());
  }
  Ok(attempt_starts(&db.events(run.id()).map_err(run_store_error)?))
}

/// When `run` ended, if it did: succeeded, failed or was cancelled.
fn finished_at(db: &ProjectDb, run: &Run) -> Result<Option<Timestamp>, StoreError> {
  if !run.status().is_terminal() {
    return Ok(None);
  }
  let events = db.events(run.id()).map_err(run_store_error)?;
  Ok(
    events
      .iter()
      .rev()
      .find(|event| {
        matches!(
          event.kind(),
          RunEventKind::Succeeded | RunEventKind::Failed { .. } | RunEventKind::Cancelled
        )
      })
      .map(RunEvent::at),
  )
}

/// Who executes a step (its agent role, or its tool's name), what it reaches, its model and its
/// permissions in the run.
type Executed = (Option<Arc<str>>, Reached, Option<Arc<str>>, Option<RunPermissions>);

/// [`Executed`] of step `key`.
fn executed(
  db: &ProjectDb,
  run: (&Run, &[Invocation]),
  key: &StepKey,
  executor: Option<&StepExecutor>,
) -> Result<Executed, StoreError> {
  Ok(match executor {
    Some(StepExecutor::Agent { profile, model, .. }) => {
      let (role, reached, permissions) = agent_step(db, run, key, *profile)?;
      (role, reached, Some(Arc::<str>::from(model.as_str())), permissions)
    }
    Some(StepExecutor::Tool { tool }) => {
      let name = db
        .resource(tool.resource)?
        .map(|tool| Arc::<str>::from(tool.name().as_str()));
      (name, Reached::default(), None, None)
    }
    None => (None, Reached::default(), None, None),
  })
}

/// The agent CLI of a connection, for a native one the base URL of its model API, and who receives the
/// prompts.
#[derive(Default)]
struct Reached {
  provider: Option<AgentProvider>,
  endpoint: Option<Arc<str>>,
  vendor: Option<EngineVendor>,
}

/// An agent step's role, CLI and model API, and permissions.
type AgentStep = (Option<Arc<str>>, Reached, Option<RunPermissions>);

/// The agent role, the CLI and the permissions of agent step `step`, which runs on `profile`; none
/// of them once the profile version is gone.
fn agent_step(
  db: &ProjectDb,
  run: (&Run, &[Invocation]),
  step: &StepKey,
  profile: PinnedVersion,
) -> Result<AgentStep, StoreError> {
  let Some(version) = db.version::<AgentProfile>(profile.resource, profile.number)? else {
    return Ok((None, Reached::default(), None));
  };
  let role = Arc::<str>::from(version.body().role().as_str());
  let reached = reached(db, version.body().connection())?;
  let permissions = run_permissions(db, run, step, (profile.resource, &version), reached.provider)?;
  Ok((Some(role), reached, permissions))
}

fn reached(db: &ProjectDb, connection: ConnectionId) -> Result<Reached, StoreError> {
  let Some(connection) = db.connection(connection)? else {
    return Ok(Reached::default());
  };
  let endpoint = connection.endpoint().api().map(|api| api.base_url().as_str().into());
  // Another kind of connection is no agent's: dropping the conversion error leaves it out.
  let Ok(agent) = AgentConnection::try_from(connection) else {
    return Ok(Reached::default());
  };
  Ok(Reached {
    provider: Some(agent.provider()),
    endpoint,
    vendor: Some(agent.vendor()),
  })
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

fn first_run_hold(db: &ProjectDb, run: &Run) -> Result<(bool, Option<GroupHold>), StoreError> {
  let first = db.run_with_key(run.task().first_run_key()).map_err(run_store_error)?;
  let is_first = first.is_some_and(|first| first.id() == run.id());
  let hold = if is_first {
    db.task_group_for_task(run.task())?
      .map(|group| db.task_group_hold(group.key()))
      .transpose()?
      .flatten()
  } else {
    None
  };
  Ok((is_first, hold))
}
