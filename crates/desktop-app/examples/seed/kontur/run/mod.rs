//! Runs of the QA project, recorded through core's run transitions the way the runner records them: a run
//! starts in a real worktree on its own branch, each step a person sees run has an invocation of its
//! agent (or of its tool's program) with a short transcript and its usage, a write step commits its work
//! as a checkpoint, and the run ends where the scenario leaves it.

mod plan_limit;

use std::collections::BTreeMap;

use orchester_core::{
  connection::stream::{AgentEvent, InvocationOutcome, ToolCommandResult},
  conversation::{AgentMessage, Message, MessageBody},
  id::{IdempotencyKey, StepExecutionId},
  resource::{ModelId, RepositoryAccess, StepKey, StepKind},
  run::{
    Answerer, BranchName, InterruptCause, Invocation, InvocationKind, NewRun, PendingToolApproval, Plan, PlanDecision,
    ReviewNote, Run, RunEvent, RunLimit, RunStatus, StepFailure, StepStatus, StuckDecision, TokenUsage, WaitReason,
  },
  store::{ActivityStore, ConversationStore, MergeStore, RunStore, WorktreeStore},
  task::Task,
  time::Timestamp,
  tokens::TokensUsed,
};

use super::{
  Kontur, SeedError,
  catalogue::{self, Published},
  git::{self, MAIN, Worktree},
};

/// What each recorded action takes, so the run reads as minutes of work.
const ACTION_MILLIS: i64 = 2 * 60 * 1000;

/// A run of one task being recorded.
pub struct Drive<'k> {
  kontur: &'k mut Kontur,
  task: Task,
  run: Run,
  at: Timestamp,
  worktree: Option<Worktree>,
  /// Invocations of the steps at work, by step.
  open: BTreeMap<StepKey, Invocation>,
}

impl<'k> Drive<'k> {
  /// Starts a run of `task`, queued on `pipeline`, at `at`, in a new worktree from the repository's first
  /// commit on `main`.
  pub fn start(kontur: &'k mut Kontur, task: &Task, pipeline: &Published, at: Timestamp) -> Result<Self, SeedError> {
    let main = BranchName::new(MAIN).expect("main is a valid branch name");
    Self::start_from(kontur, (task, pipeline), at, Some(main))
  }

  /// [`Drive::start`] from a checkout with a detached HEAD: the run records no branch to merge into.
  pub fn start_detached(
    kontur: &'k mut Kontur,
    task: &Task,
    pipeline: &Published,
    at: Timestamp,
  ) -> Result<Self, SeedError> {
    Self::start_from(kontur, (task, pipeline), at, None)
  }

  fn start_from(
    kontur: &'k mut Kontur,
    (task, pipeline): (&Task, &Published),
    at: Timestamp,
    base_branch: Option<BranchName>,
  ) -> Result<Self, SeedError> {
    let earlier = kontur.db.runs_of_task(task.id())?.len();
    let limits = task.details().limits;
    let new = NewRun {
      task: task.id(),
      snapshot: pipeline.snapshot().clone(),
      branch: BranchName::for_next_run(task.key(), earlier),
      base: kontur.base.clone(),
      base_branch,
      budget: limits.budget(),
      max_duration: limits.max_duration(),
    };
    let mut run = kontur
      .db
      .create_run_of_queued_task(IdempotencyKey::generate(), new, at)?;
    let events = run.start(at)?;
    kontur.db.append_events(&run, &events)?;
    let worktree = Worktree::add(&kontur.repo, &run.id().to_string(), run.branch().as_str(), &kontur.base)?;
    Ok(Self {
      kontur,
      task: task.clone(),
      run,
      at,
      worktree: Some(worktree),
      open: BTreeMap::new(),
    })
  }

  pub fn run(&self) -> &Run {
    &self.run
  }

  fn tick(&mut self) -> Timestamp {
    self.at = Timestamp::from_unix_millis(self.at.unix_millis() + ACTION_MILLIS);
    self.at
  }

  fn record(&mut self, events: &[RunEvent]) -> Result<(), SeedError> {
    self.kontur.db.append_events(&self.run, events)?;
    Ok(())
  }

  fn key(step: &str) -> StepKey {
    StepKey::new(step).expect("scenarios name the catalogue's step keys")
  }

  /// The latest execution of `step`.
  fn execution(&self, step: &str) -> Result<StepExecutionId, SeedError> {
    self
      .run
      .latest_execution(&Self::key(step))
      .map(|execution| execution.id())
      .ok_or_else(|| SeedError::NoSuchStep(step.to_owned()))
  }

  fn kind(&self, step: &str) -> Result<&StepKind, SeedError> {
    self
      .run
      .snapshot()
      .body()
      .step(&Self::key(step))
      .map(|pinned| &pinned.kind)
      .ok_or_else(|| SeedError::NoSuchStep(step.to_owned()))
  }

  fn writes(&self, step: &str) -> Result<bool, SeedError> {
    Ok(matches!(
      self.kind(step)?,
      StepKind::Agent {
        repository: RepositoryAccess::Write,
        ..
      }
    ))
  }

  /// Starts `step` and its agent's (or tool's) invocation.
  pub fn begin(&mut self, step: &str) -> Result<&mut Self, SeedError> {
    let execution = self.execution(step)?;
    let at = self.tick();
    let events = self.run.start_step(execution, at)?;
    self.record(&events)?;
    let tool = matches!(self.kind(step)?, StepKind::Tool { .. });
    let db = &mut self.kontur.db;
    let invocation = if tool {
      db.start_command_invocation(self.run.id(), execution, at)?
    } else {
      db.start_invocation(
        self.run.id(),
        execution,
        &ModelId::new(catalogue::MODEL).expect("a valid model"),
        at,
      )?
    };
    let opening = if tool {
      vec![AgentEvent::ToolCommandStarted {
        tool: step.to_owned(),
        command: catalogue::command_line(step),
      }]
    } else {
      vec![
        AgentEvent::SessionStarted {
          session: format!("thread-{}", invocation.id),
        },
        AgentEvent::TurnStarted,
        AgentEvent::Message {
          text: format!("Starting {step} for {}.", self.task.key()),
        },
      ]
    };
    db.append_activity(&invocation, &stamped(at, opening))?;
    self.open.insert(Self::key(step), invocation);
    Ok(self)
  }

  /// Ends the open invocation of `step` as `outcome`, after `events`.
  fn end(&mut self, step: &str, events: Vec<AgentEvent>, outcome: InvocationOutcome) -> Result<(), SeedError> {
    let mut invocation = self.open.remove(&Self::key(step)).ok_or(SeedError::NotAtWork)?;
    let at = self.tick();
    let usage = match (&invocation.kind, &outcome) {
      (InvocationKind::Agent { .. }, InvocationOutcome::Completed) => usage_of(step),
      _ => TokenUsage::default(),
    };
    let mut events = events;
    if usage != TokenUsage::default() {
      events.push(AgentEvent::TurnCompleted { usage });
    }
    self.kontur.db.append_activity(&invocation, &stamped(at, events))?;
    invocation.ended_at = Some(at);
    invocation.usage = usage;
    invocation.outcome = Some(outcome);
    self.kontur.db.save_invocation(&invocation)?;
    if usage != TokenUsage::default() {
      let events = self.run.record_usage(usage.counted_tokens(), at)?;
      self.record(&events)?;
    }
    Ok(())
  }

  /// The closing events of a step that did its work.
  fn done(&self, step: &str) -> Result<Vec<AgentEvent>, SeedError> {
    Ok(match self.kind(step)? {
      StepKind::Tool { .. } => vec![AgentEvent::ToolCommandFinished {
        tool: step.to_owned(),
        command: catalogue::command_line(step),
        result: ToolCommandResult::Passed,
        exit_code: Some(0),
        output: "test result: ok. 42 passed; 0 failed".to_owned(),
        omitted_bytes: 0,
      }],
      StepKind::Agent { .. } => vec![AgentEvent::Message {
        text: format!("Finished {step}."),
      }],
    })
  }

  /// Runs `step` to its pass: a write step commits a note as its checkpoint, a gated one has its plan
  /// approved by the person.
  pub fn pass(&mut self, step: &str) -> Result<&mut Self, SeedError> {
    let attempt = self.run.current_attempt().map_or(1, |attempt| attempt.number().get());
    let note = format!("{}: {step}, attempt {attempt}\n", self.task.details().title.as_str());
    let notes = self.notes();
    self.pass_with(step, &[(notes.as_str(), note)])
  }

  /// [`Drive::pass`] with `files` as a write step's checkpoint.
  pub fn pass_with(&mut self, step: &str, files: &[(&str, String)]) -> Result<&mut Self, SeedError> {
    if !self.open.contains_key(&Self::key(step)) {
      self.begin(step)?;
    }
    if self.writes(step)? {
      self.checkpoint(step, files)?;
    }
    let gated = self
      .run
      .snapshot()
      .body()
      .step(&Self::key(step))
      .is_some_and(|pinned| pinned.approval_gate);
    if gated {
      let mut plan = self.propose(step, &format!("1. Change what {step} found.\n2. Cover it with tests."))?;
      let at = self.tick();
      let events = self
        .run
        .decide_plan(&mut plan, PlanDecision::Approved, Answerer::Person, at)?;
      self.kontur.db.append_events_with_plan(&self.run, &events, &plan)?;
      return Ok(self);
    }
    let closing = self.done(step)?;
    self.end(step, closing, InvocationOutcome::Completed)?;
    let execution = self.execution(step)?;
    let at = self.tick();
    let events = self.run.complete_step(execution, at)?;
    self.record(&events)?;
    Ok(self)
  }

  /// Passes every step listed before `step` whose latest execution has not passed.
  pub fn pass_before(&mut self, step: &str) -> Result<&mut Self, SeedError> {
    self.pass_listed(Some(step))
  }

  /// Passes every step whose latest execution has not passed: the run succeeds.
  pub fn pass_all(&mut self) -> Result<&mut Self, SeedError> {
    self.pass_listed(None)
  }

  /// Passes the steps listed before `until` (all of them without it) that have not passed, in order.
  fn pass_listed(&mut self, until: Option<&str>) -> Result<&mut Self, SeedError> {
    let listed: Vec<String> = self
      .run
      .snapshot()
      .body()
      .steps()
      .iter()
      .map(|pinned| pinned.key.as_str().to_owned())
      .take_while(|key| Some(key.as_str()) != until)
      .collect();
    for key in listed {
      let passed = self
        .run
        .latest_execution(&Self::key(&key))
        .is_some_and(|execution| execution.status() == StepStatus::Passed);
      if !passed {
        self.pass(&key)?;
      }
    }
    Ok(self)
  }

  /// Commits the work of `step`, at work, as a checkpoint when it is a write step.
  pub fn save_work(&mut self, step: &str) -> Result<&mut Self, SeedError> {
    if !self.writes(step)? {
      return Ok(self);
    }
    let attempt = self.run.current_attempt().map_or(1, |attempt| attempt.number().get());
    let note = format!(
      "{}: {step} so far, attempt {attempt}\n",
      self.task.details().title.as_str()
    );
    let notes = self.notes();
    self.checkpoint(step, &[(notes.as_str(), note)])
  }

  /// The file a write step's checkpoint changes unless the scenario gives its files: one per task, so
  /// merging one task's run never conflicts with another's.
  fn notes(&self) -> String {
    format!("notes/{}.md", self.task.key().to_string().to_lowercase())
  }

  /// Commits `files` in the run's worktree and records the commit as `step`'s checkpoint.
  pub fn checkpoint(&mut self, step: &str, files: &[(&str, String)]) -> Result<&mut Self, SeedError> {
    let worktree = self.worktree.as_ref().ok_or(SeedError::WorktreeRemoved)?;
    let revision = worktree.commit(files, &format!("{}: {step}", self.task.key()))?;
    let execution = self.execution(step)?;
    let at = self.tick();
    let events = self.run.record_checkpoint(execution, revision, at)?;
    self.record(&events)?;
    Ok(self)
  }

  /// Leaves `files` in the worktree uncommitted: work since the last checkpoint.
  pub fn uncommitted(&mut self, files: &[(&str, String)]) -> Result<&mut Self, SeedError> {
    self.worktree.as_ref().ok_or(SeedError::WorktreeRemoved)?.leave(files)?;
    Ok(self)
  }

  /// `step`'s agent proposes `text` as its next plan, and the step waits for a decision on it.
  pub fn propose(&mut self, step: &str, text: &str) -> Result<Plan, SeedError> {
    if !self.open.contains_key(&Self::key(step)) {
      self.begin(step)?;
    }
    let closing = vec![AgentEvent::Message { text: text.to_owned() }];
    self.end(step, closing, InvocationOutcome::Completed)?;
    let execution = self.execution(step)?;
    let earlier = self.kontur.db.plans(self.run.id())?;
    let at = self.tick();
    let (events, plan) = self.run.propose_plan(execution, text.to_owned(), &earlier, at)?;
    self.kontur.db.append_events_with_plan(&self.run, &events, &plan)?;
    Ok(plan)
  }

  /// The person asks for changes to `plan` with `note`; the step's agent works on it again.
  pub fn request_changes(&mut self, step: &str, mut plan: Plan, note: &str) -> Result<&mut Self, SeedError> {
    let note = ReviewNote::new(note).map_err(|error| SeedError::Invalid(error.to_string()))?;
    let at = self.tick();
    let events = self
      .run
      .decide_plan(&mut plan, PlanDecision::ChangesRequested { note }, Answerer::Person, at)?;
    self.kontur.db.append_events_with_plan(&self.run, &events, &plan)?;
    let execution = self.execution(step)?;
    let invocation = self.kontur.db.start_invocation(
      self.run.id(),
      execution,
      &ModelId::new(catalogue::MODEL).expect("a valid model"),
      at,
    )?;
    self.open.insert(Self::key(step), invocation);
    Ok(self)
  }

  /// `step`, at work, fails for `reason`, and the run follows its failure route.
  pub fn fail(&mut self, step: &str, reason: StepFailure) -> Result<&mut Self, SeedError> {
    if !self.open.contains_key(&Self::key(step)) {
      self.begin(step)?;
    }
    let closing = match self.kind(step)? {
      StepKind::Tool { .. } => vec![AgentEvent::ToolCommandFinished {
        tool: step.to_owned(),
        command: catalogue::command_line(step),
        result: ToolCommandResult::Failed,
        exit_code: Some(101),
        output: "test webhooks::retry::jitter_bound ... FAILED\ntest result: FAILED. 41 passed; 1 failed".to_owned(),
        omitted_bytes: 0,
      }],
      StepKind::Agent { .. } => vec![AgentEvent::Message {
        text: "The change does not meet the acceptance criteria yet.".to_owned(),
      }],
    };
    let outcome = match self.kind(step)? {
      StepKind::Tool { .. } => InvocationOutcome::ExitFailed { code: Some(101) },
      StepKind::Agent { .. } => InvocationOutcome::Completed,
    };
    self.end(step, closing, outcome)?;
    let execution = self.execution(step)?;
    let at = self.tick();
    let events = self.run.fail_step(execution, reason, at)?;
    self.record(&events)?;
    Ok(self)
  }

  /// Fails `step` with a verdict until no automatic route is left: the run waits for a person.
  pub fn fail_until_stuck(&mut self, step: &str) -> Result<&mut Self, SeedError> {
    let verdict = match self.kind(step)? {
      StepKind::Tool { .. } => StepFailure::CommandFailed,
      StepKind::Agent { .. } => StepFailure::ReportedFailed,
    };
    while self.run.status() == &RunStatus::Running {
      self.pass_before(step)?.fail(step, verdict)?;
    }
    Ok(self)
  }

  /// `step`'s agent, at work, asks `question`: its work is committed and the step waits for the answer.
  pub fn ask(&mut self, step: &str, question: &str) -> Result<&mut Self, SeedError> {
    let invocation = self.open.get(&Self::key(step)).ok_or(SeedError::NotAtWork)?.id;
    let execution = self.execution(step)?;
    self.end(step, Vec::new(), InvocationOutcome::Completed)?;
    let body = MessageBody::new(question).map_err(|error| SeedError::Invalid(error.to_string()))?;
    let at = self.tick();
    let message = Message::from_agent(
      self.task.id(),
      self.run.id(),
      (execution, invocation),
      AgentMessage::Question,
      body,
      at,
    );
    self.kontur.db.add_agent_message(&message)?;
    let events = self.run.wait_step(execution, WaitReason::Message, at)?;
    self.record(&events)?;
    Ok(self)
  }

  /// `step`'s agent, at work, asks to run `command`, which needs a person's approval.
  pub fn approval(&mut self, step: &str, command: &str, agent: &str) -> Result<&mut Self, SeedError> {
    let invocation = self.open.get(&Self::key(step)).ok_or(SeedError::NotAtWork)?.id;
    let at = self.tick();
    let request = PendingToolApproval {
      id: format!("{invocation}:call_1"),
      run: self.run.id(),
      execution: self.execution(step)?,
      invocation,
      agent: agent.to_owned(),
      command: true,
      scope: Some("Adds the dependency the retry schedule needs.".to_owned()),
      server: "command".to_owned(),
      action: command.to_owned(),
      target: None,
      requested_at: at,
      native: None,
    };
    self.kontur.db.request_tool_approval(&request)?;
    Ok(self)
  }

  /// Ends the invocations at work, which the runner stops when a run pauses, stops or is cancelled.
  fn stop_open(&mut self) -> Result<(), SeedError> {
    let steps: Vec<String> = self.open.keys().map(|key| key.as_str().to_owned()).collect();
    for step in steps {
      self.end(&step, Vec::new(), InvocationOutcome::Terminated)?;
    }
    Ok(())
  }

  pub fn pause(&mut self) -> Result<&mut Self, SeedError> {
    self.stop_open()?;
    let at = self.tick();
    let events = self.run.pause(at)?;
    self.record(&events)?;
    Ok(self)
  }

  /// Stops at `limit`; a run's own token limit is used up first, as the runner stops it when it is.
  pub fn stop_at_limit(&mut self, limit: RunLimit) -> Result<&mut Self, SeedError> {
    self.stop_open()?;
    let at = self.tick();
    if limit == RunLimit::Tokens {
      let budget = self.run.budget().ok_or(SeedError::NoBudget)?;
      let left = budget.get().saturating_sub(self.run.used().get());
      let events = self.run.record_usage(TokensUsed::new(left), at)?;
      self.record(&events)?;
    }
    let events = self.run.stop_at_limit(limit, at)?;
    self.record(&events)?;
    Ok(self)
  }

  /// The app ended while the run worked: no end of its invocations was recorded.
  pub fn interrupt(&mut self) -> Result<&mut Self, SeedError> {
    let at = self.tick();
    let events = self.run.interrupt(InterruptCause::Crashed, at)?;
    self.record(&events)?;
    Ok(self)
  }

  pub fn cancel(&mut self) -> Result<&mut Self, SeedError> {
    self.stop_open()?;
    let at = self.tick();
    let events = self.run.cancel(at)?;
    self.record(&events)?;
    Ok(self)
  }

  /// The person stops a run that waits for them: it fails.
  pub fn stop(&mut self) -> Result<&mut Self, SeedError> {
    let at = self.tick();
    let events = self.run.decide_stuck(&StuckDecision::Stop, at)?;
    self.record(&events)?;
    Ok(self)
  }

  /// Merge into main, then Remove worktree.
  pub fn merge(&mut self) -> Result<&mut Self, SeedError> {
    let at = self.tick();
    let merge = git::merge(&self.kontur.repo, (&self.run, self.task.key()), at)?;
    self.kontur.db.save_run_merge(&merge)?;
    self.remove_worktree()
  }

  /// Removes the run's worktree; its branch and commits stay.
  pub fn remove_worktree(&mut self) -> Result<&mut Self, SeedError> {
    let worktree = self.worktree.take().ok_or(SeedError::WorktreeRemoved)?;
    worktree.remove()?;
    let at = self.tick();
    self.kontur.db.save_worktree_removal(self.run.id(), at)?;
    Ok(self)
  }

  /// The run as recorded.
  pub fn finish(self) -> Run {
    self.run
  }
}

fn stamped(at: Timestamp, events: Vec<AgentEvent>) -> Vec<(Timestamp, AgentEvent)> {
  events.into_iter().map(|event| (at, event)).collect()
}

/// What a step's agent reported using: tens of thousands of tokens, more for the steps that read more.
fn usage_of(step: &str) -> TokenUsage {
  let input = match step {
    "research" => 64_000,
    "implement" => 38_000,
    "review" => 22_000,
    _ => 12_000,
  };
  TokenUsage {
    input,
    cached_input: input / 2,
    output: input / 8,
  }
}
