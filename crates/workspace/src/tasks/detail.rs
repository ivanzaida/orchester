//! A task with its latest run, as the task page (02.10 `RD4pc`, 02.I `SfXU1`) shows it: the
//! run's steps in pipeline order with the agent and model each runs on, and the run's CLI
//! invocations. Read from the project database, which the runner writes; blocks on SQLite, so
//! the UI calls it off the UI thread.

use std::{collections::BTreeMap, num::NonZeroU32, path::Path, sync::Arc};

use orchester_core::{
  attachment::Attachment,
  connection::{AgentProvider, Effort, EngineVendor},
  conversation::Message,
  id::{InvocationId, MessageId, RunId, StepExecutionId, TaskId},
  integration::RunMerge,
  question::AskedQuestions,
  resource::{StepKey, VersionNumber},
  run::{
    ActiveClock, ActivityEntry, AttemptStart, Invocation, PauseReason, Plan, Run, RunLimit, RunStatus, RunnerFailure,
    StepExecutor, StepFailure, StepOutput, StepStatus, TaskPhase, TokenUsage, active_time, execution_tokens,
    review::{ReviewRecord, reviews_usage},
  },
  store::{ActivityStore, ConversationStore, MergeStore, ResourceStore, RunStore, TaskStore, WorktreeStore},
  task::{Task, requirements::TaskRequirements},
  task_group::GroupHold,
  time::Timestamp,
  tokens::{TokenBudget, TokensUsed},
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

pub use super::permissions::{ChangeState, RunPermissions};
use super::{
  delivery::{self, Delivery},
  engines,
  history::{self, RunRecord},
  index_use::IndexUses,
  requirements::LocalRuns,
  run_approvals::RunApprovals,
  run_times::{finished_at, paused_at, starts_of},
  step_executors::{Reached, executed},
  unreadable::Unreadable,
};

/// A task's messages, oldest first, and the files attached to them.
type Conversation = (Vec<Message>, BTreeMap<MessageId, Vec<Attachment>>);

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
  /// The base URL of a native agent's model API: as its latest invocation recorded it, else as the
  /// connection is now.
  pub endpoint: Option<Arc<str>>,
  /// Who receives the agent's prompts, as its engine is named (core's
  /// [`orchester_core::connection::AgentConnection::vendor`]): as a native agent's latest invocation
  /// recorded it, else by that connection as it is now.
  pub vendor: Option<EngineVendor>,
  /// Whether `endpoint` and `vendor` are what the latest invocation used, rather than the connection now.
  pub engine_recorded: bool,
  /// The effort a native agent's latest invocation ran with.
  pub effort: Option<Effort>,
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

impl StepView {
  /// Whether the step's agent may write in the worktree in its next invocation, as the run's pinned pipeline version,
  /// its profile version and any change of its permissions for this run give them.
  pub fn writes(&self) -> bool {
    self.permissions.as_ref().is_some_and(|permissions| {
      permissions.permissions.repository() == orchester_core::permission::RepositoryAccess::Write
    })
  }
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
  /// The account each agent CLI invocation started on, in the order they started.
  pub accounts: Vec<(orchester_core::id::InvocationId, orchester_core::id::AgentAccountId)>,
  /// What auto mode's reviews of its requests cost, in the order they ended.
  pub reviews: Vec<ReviewRecord>,
  /// When a paused run paused or stopped at its limit.
  pub paused_at: Option<Timestamp>,
  /// The run's work was committed as a checkpoint right before it paused.
  pub saved_at_pause: bool,
  /// When a finished run (succeeded, failed or cancelled) ended.
  pub finished_at: Option<Timestamp>,
  /// When its own worktree was removed, if Orchester removed it: an interrupted run then can never
  /// continue (ORC-63).
  pub worktree_removed: Option<Timestamp>,
  /// Why the runner failed, for a run it interrupted so and recorded why (ORC-75).
  pub runner_failure: Option<RunnerFailure>,
  /// What its step executions left for the steps after them.
  pub outputs: Vec<StepOutput>,
  /// The plans of its gated steps, by step and version.
  pub plans: Vec<Plan>,
  /// What its agent launches got of the code index and used before their first edit.
  pub index: IndexUses,
  /// When each of its attempts started, and whether a person's decision started it; empty for a
  /// run of one attempt.
  pub attempt_starts: Vec<AttemptStart>,
  /// Its auto-approval, its rules from the wider answers and how its requests were answered.
  pub approvals: RunApprovals,
  /// Its agents' questions with suggested answers that wait for the person.
  pub waiting_questions: Vec<AskedQuestions>,
  /// The notices that its base branch moved, oldest first (`orchester_core::base_move`).
  pub base_notices: Vec<orchester_core::base_move::BaseNotice>,
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

  /// Tokens the CLIs and auto mode's reviewer reported for the whole run.
  pub fn usage(&self) -> TokenUsage {
    let mut usage: TokenUsage = self.invocations.iter().map(|invocation| invocation.usage).sum();
    usage += reviews_usage(&self.reviews);
    usage
  }

  /// The reviews of `invocation`'s requests.
  pub fn reviews_of(&self, invocation: InvocationId) -> impl Iterator<Item = &ReviewRecord> {
    self
      .reviews
      .iter()
      .filter(move |review| review.invocation == invocation)
  }

  /// Active agent time of the run so far: pauses and waits for a person do not count.
  pub fn active_time(&self, now: Timestamp) -> std::time::Duration {
    active_time(&self.invocations, now)
  }

  /// Active agent time of `invocations` of the run at `now`, counting on while the run lets it,
  /// but not while it has no runner (`lost`) to look after its agents.
  pub fn active_clock<'a>(
    &self,
    invocations: impl IntoIterator<Item = &'a Invocation>,
    now: Timestamp,
    lost: bool,
  ) -> ActiveClock {
    let clock = ActiveClock::of(invocations, now).within(&self.run);
    if lost { clock.stopped() } else { clock }
  }

  /// The step a run stopped at its limit was working on, if it stopped mid-step: after a step's
  /// budget, that step.
  pub fn stopped_step(&self) -> Option<&StepView> {
    if let Some(execution) = self.stopped_execution() {
      return self.step_of(execution);
    }
    self.steps.iter().find(|step| step.status == Some(StepStatus::Running))
  }

  /// The execution whose step budget or account's plan limit the run stopped at, if it did.
  pub fn stopped_execution(&self) -> Option<StepExecutionId> {
    match self.run.status() {
      RunStatus::Paused(PauseReason::LimitReached(
        RunLimit::StepBudget { execution } | RunLimit::PlanLimit { execution },
      )) => Some(*execution),
      _ => None,
    }
  }

  /// What `execution` used of its step's budget in force for the run, over all its invocations and
  /// the reviews of their requests; `None` when its step has no budget.
  pub fn step_budget_use(&self, execution: StepExecutionId) -> Option<(TokensUsed, TokenBudget)> {
    let budget = self.run.step_budget(self.run.execution(execution)?.step())?;
    Some((execution_tokens(&self.invocations, &self.reviews, execution), budget))
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
  /// What the task asked for when it was created, when its latest run started and now (Details).
  pub requirements: TaskRequirements,
  /// The latest run, when it can be read.
  pub run: Option<RunView>,
  /// The merge of the latest run into its base branch, once it was merged.
  pub merge: Option<RunMerge>,
  /// How the latest run is delivered when its pipeline ends in a Deliver step, with its place in the project's merge
  /// queue once it joined it (`crate::merge_queue`).
  pub queue: Option<crate::merge_queue::TaskDelivery>,
  /// What became of the latest run's work when it passed every step and Orchester did not merge it, as git
  /// tells it ([`Self::load_settled`]); `Waiting` as far as [`Self::load_in`] knows.
  pub delivery: Delivery,
  /// For a latest run that passed every step: what it delivers, its completion, its report and the tool calls it made
  /// outside the repository (ORC-93, [`super::completion`]).
  pub result: Option<super::completion::RunResult>,
  /// The task's conversation, oldest first.
  pub messages: Vec<Message>,
  /// The files people attached to its messages, by message, in the order they were attached.
  pub attachments: BTreeMap<MessageId, Vec<Attachment>>,
  /// The latest run, when it cannot be read (02.R `SUuwV`): `run` is `None`.
  pub latest_unreadable: Option<Unreadable>,
  /// All the task's runs, newest first: *Runs of this task* (ORC-63).
  pub history: Vec<RunRecord>,
  /// The task's earlier runs that cannot be read, oldest first, while its latest run can be.
  pub earlier_unreadable: Vec<Unreadable>,
  /// The keys of the linked blocking tasks that have not finished: while there are any, the
  /// task's run neither starts nor goes on.
  pub blocked_by: Vec<String>,
  /// A shared task's pipeline, by its slug, that this computer has no pipeline for.
  pub missing_pipeline: Option<String>,
  /// A run's workflow status of this shared task that was not sent to its server, and why.
  pub unsent_status: Option<orchester_store::UnsentStatus>,
  /// How the task stands on its server, for a task of a shared project or one kept *Not shared*.
  pub shared: Option<orchester_store::SharedTaskInfo>,
  /// Before the task has a run: the agent its run would start with.
  pub first_agent: Option<Arc<str>>,
}

impl TaskDetail {
  /// Where the task stands, from its latest run; as for a task without runs while that run cannot
  /// be read.
  pub fn phase(&self) -> TaskPhase {
    self.delivery.phase(TaskPhase::of(
      &self.task,
      self.run.as_ref().map(|run| &run.run),
      self.merge.as_ref(),
    ))
  }

  /// [`Self::load_in`] with what git, through `programs`, tells of the runs that passed every step and that
  /// Orchester did not merge ([`super::delivery`]): the latest as the task list's rows read it, and each of
  /// [`Self::history`].
  pub fn load_settled(repo_root: &Path, task: TaskId, programs: &Programs) -> Result<Self, StoreError> {
    let mut detail = Self::load_in(repo_root, task)?;
    let settled = delivery::settle_records(programs, repo_root, detail.history.iter_mut());
    if detail.phase() == TaskPhase::Succeeded
      && let Some(run) = &detail.run
      && let Some((_, latest)) = settled.into_iter().find(|(id, _)| *id == run.run.id())
    {
      detail.delivery = latest;
    }
    Ok(detail)
  }

  /// Whether the task can be edited: a draft or a task with a recorded run, unless it was kept
  /// *Not shared*, which is read-only.
  pub fn editable(&self) -> bool {
    let has_run = self.run.is_some() || self.latest_unreadable.is_some();
    !self.not_shared() && self.task.check_editable(has_run).is_ok()
  }

  /// Whether the task was kept *Not shared* when its project joined: read-only until *Share task*.
  pub fn not_shared(&self) -> bool {
    self.shared.as_ref().is_some_and(|info| info.kept_tags.is_some())
  }

  /// Whether the run is recorded as running while no runner is `online`: its agent is gone and
  /// the run is interrupted until a runner is back ([`TaskPhase::without_runner`]).
  pub fn lost_its_runner(&self, online: bool) -> bool {
    let phase = self.phase();
    !online && phase.without_runner() != phase
  }

  pub fn load_in(repo_root: &Path, task: TaskId) -> Result<Self, StoreError> {
    let mut db = ProjectDb::open(repo_root)?;
    let task = db.task(task)?.ok_or(StoreError::TaskNotFound(task))?;
    let blocked_by = blocking_keys(&db, &task)?;
    let missing_pipeline = db.missing_pipelines()?.remove(&task.id());
    let unsent_status = db.unsent_status(task.id())?;
    let shared = db.shared_task_info(task.id())?;
    let mut runs = db.runs_of_task(task.id()).map_err(run_store_error)?;
    history::note_missing_worktrees(&mut db, repo_root, &runs)?;
    let conversation = (db.messages(task.id())?, db.message_attachments(task.id())?);
    let history = history::read(&db, &task, &runs)?;
    let latest = runs.pop();
    if let Some(Err(latest)) = &latest {
      let local = LocalRuns {
        latest: None,
        unreadable: true,
      };
      let requirements = super::requirements::read(&db, &task, local, shared.as_ref())?;
      return Ok(Self {
        latest_unreadable: Some(Unreadable::of(latest)),
        history,
        blocked_by,
        missing_pipeline,
        unsent_status,
        shared,
        ..Self::without_runs(task, conversation, requirements)
      });
    }
    let earlier_unreadable = runs
      .iter()
      .filter_map(|run| run.as_ref().err())
      .map(Unreadable::of)
      .collect();
    let latest = latest.and_then(Result::ok);
    let local = LocalRuns {
      latest: latest.as_ref(),
      unreadable: false,
    };
    let requirements = super::requirements::read(&db, &task, local, shared.as_ref())?;
    let run_task = latest
      .as_ref()
      .map(|run| db.task_for_run(task.id(), run.id()))
      .transpose()?
      .flatten();
    let (merge, queue) = match &latest {
      Some(run) => (db.run_merge(run.id())?, crate::merge_queue::task_delivery(&db, run)?),
      None => (None, None),
    };
    let first_agent = match latest {
      None => super::first_agent::first_agent(&db, &task)?,
      Some(_) => None,
    };
    let run = latest.map(|run| run_view(&db, run)).transpose()?;
    let result = run
      .as_ref()
      .filter(|view| *view.run.status() == RunStatus::Succeeded)
      .map(|view| super::completion::RunResult::read(&db, &task, &view.run, (&view.outputs, &view.plans)))
      .transpose()?;
    Ok(Self {
      run,
      result,
      first_agent,
      run_task,
      merge,
      queue,
      history,
      earlier_unreadable,
      blocked_by,
      missing_pipeline,
      unsent_status,
      shared,
      ..Self::without_runs(task, conversation, requirements)
    })
  }

  fn without_runs(task: Task, (messages, attachments): Conversation, requirements: TaskRequirements) -> Self {
    Self {
      task,
      run_task: None,
      requirements,
      run: None,
      merge: None,
      queue: None,
      delivery: Delivery::Waiting,
      result: None,
      messages,
      attachments,
      latest_unreadable: None,
      history: Vec::new(),
      earlier_unreadable: Vec::new(),
      blocked_by: Vec::new(),
      missing_pipeline: None,
      unsent_status: None,
      shared: None,
      first_agent: None,
    }
  }
}

/// The keys of `task`'s unfinished blocking tasks that are in the project: a shared project's first download brings its
/// tasks page by page, so a blocker may not have arrived yet. A run stays refused until it finished, arrived or not.
fn blocking_keys(db: &ProjectDb, task: &Task) -> Result<Vec<String>, StoreError> {
  let mut keys = Vec::new();
  for blocker in db.unfinished_blocking_tasks(task.id())? {
    if let Some(blocker) = db.task(blocker)? {
      keys.push(blocker.key().to_string());
    }
  }
  Ok(keys)
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
      engine_recorded: false,
      effort: None,
      model,
      permissions,
      execution: latest.map(|execution| execution.id()),
      status: latest.map(|execution| execution.status()),
      attempt,
      failure: latest.and_then(|execution| execution.failure()),
      stage: stage_of(&step.key),
    });
  }
  engines::apply_recorded(db, &mut steps, &invocations)?;
  let (paused_at, saved_at_pause) = paused_at(db, &run)?;
  let finished_at = finished_at(db, &run)?;
  let worktree_removed = db.worktree_removal(run.id())?;
  let attempt_starts = starts_of(db, &run)?;
  Ok(RunView {
    attempt_starts,
    first_run,
    group_hold,
    approvals: RunApprovals::read(db, &run)?,
    waiting_questions: waiting_questions(db, &run)?,
    base_notices: db.run_base_notices(run.id())?,
    accounts: db.invocation_accounts(run.id())?,
    index: IndexUses::read(db, &invocations)?,
    invocations,
    reviews: db.reviewer_usage(run.id())?,
    outputs: db.step_outputs(run.id())?,
    plans: db.plans(run.id()).map_err(run_store_error)?,
    paused_at,
    saved_at_pause,
    finished_at,
    worktree_removed,
    runner_failure: history::runner_failure(db, &run)?,
    pipeline,
    version: pinned.number,
    steps,
    run,
  })
}

/// `run`'s agents' questions with suggested answers that wait for the person.
fn waiting_questions(db: &ProjectDb, run: &Run) -> Result<Vec<AskedQuestions>, StoreError> {
  Ok(
    db.questions(run.id())?
      .into_iter()
      .filter(AskedQuestions::waits)
      .collect(),
  )
}

/// The transcript of `run` as stored.
pub fn activity_in(repo_root: &Path, run: RunId) -> Result<Vec<ActivityEntry>, StoreError> {
  ProjectDb::open(repo_root)?.activity(run, 0)
}

pub fn run_store_error(error: orchester_store::RunStoreError) -> StoreError {
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
