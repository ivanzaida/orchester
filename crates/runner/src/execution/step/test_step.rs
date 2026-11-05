//! Test steps (`docs/architecture/agent-testing.md`, "The Test step"; [`orchester_core::test_step`]). A Test step works
//! where a tool step does (see `tool`): in a command slot, alone in the run's worktree or beside other steps in one of
//! its own. It reads which paths the run changed since its base revision, committed or not, and the project's
//! published suites, services and stacks, and runs the suites the core's plan chooses layer by layer, each as
//! `run_tests` runs one ([`super::suite`]): with its needs started before and stopped after, its run kept with the
//! run's storage. Each test that failed runs once more with its filter, off the async workers. Its result is the
//! step's output, and failed tests fail the step for its failure route.
//!
//! The person chose the step in the pipeline, so its suites and the services they need start without asking, as a
//! tool step's command does. A stop of the run (a pause, a limit, a cancel) takes effect between two suite runs: the
//! suite that runs then ends on its own or at its timeout first.

use std::{
  path::Path,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use orchester_core::{
  connection::stream::{InvocationOutcome, RunnerProblem},
  id::StepExecutionId,
  resource::StepKey,
  run::{Invocation, StepAt, StepExecutor, StepFailure},
  store::ActivityStore,
  test_report::SuiteRun,
  test_step::{PlannedSuite, SuiteOutcome, TestPlan, TestStepResult, rerun_filters},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use tokio::sync::mpsc;

use super::{
  Active,
  service::{StepServices, published_services, published_stacks},
  start::problem,
  suite::{StepSuites, published},
  worker::{Stop, StopCause, WorkerEvent},
};
use crate::{
  execution::{
    run::{ExecutionError, RunTask},
    tool::CommandFinished,
    workspace::Workspace,
  },
  git::Git,
};

impl RunTask {
  /// Whether `execution` belongs to a Test step.
  pub(in crate::execution) fn is_test_step(&self, execution: StepExecutionId) -> bool {
    self
      .run
      .execution(execution)
      .and_then(|execution| self.run.snapshot().step(execution.step()))
      .is_some_and(|step| matches!(step.executor(), StepExecutor::Test))
  }

  /// Starts the Test step `execution`, whose `invocation` started, in `workspace`; a plan that cannot be read fails
  /// the step as not started.
  pub(in crate::execution) fn start_tests(
    &mut self,
    execution: StepExecutionId,
    mut invocation: Invocation,
    workspace: Workspace,
  ) -> Result<(), ExecutionError> {
    let prepared = self
      .test_plan(workspace.path())
      .and_then(|plan| self.environment().map(|environment| (plan, environment)));
    let (plan, environment) = match prepared {
      Ok(prepared) => prepared,
      Err(problem) => return self.not_started(execution, &mut invocation, problem, StepFailure::NotStarted),
    };
    let step = self
      .run
      .execution(execution)
      .expect("a started Test step belongs to an execution of the run")
      .step()
      .clone();
    let (stops, stopped) = mpsc::unbounded_channel();
    let worker = TestWorker {
      root: self.root.clone(),
      invocation: invocation.clone(),
      step,
      plan,
      suites: self.step_suites(workspace.path(), &environment),
      services: self.step_services(invocation.id, workspace.path(), &environment),
      events: self.worker_events.clone(),
      stops: stopped,
    };
    tokio::spawn(worker.work());
    self
      .agents
      .insert(execution, Active::command(stops, workspace, invocation));
    Ok(())
  }

  /// The suites the run's changes in `worktree` choose, from the project's published suites, services and stacks.
  fn test_plan(&self, worktree: &Path) -> Result<TestPlan, RunnerProblem> {
    let unreadable = |what: &str, error: &dyn std::error::Error| problem(format!("cannot read {what}: {error}"));
    let suites = published(&self.root).map_err(|error| unreadable("the project's test suites", &error))?;
    let services = published_services(&self.root).map_err(|error| unreadable("the project's services", &error))?;
    let stacks = published_stacks(&self.root).map_err(|error| unreadable("the project's stacks", &error))?;
    let changed = changed_paths(worktree, self.run.base().as_str())
      .map_err(|error| unreadable("the paths the run changed", &error))?;
    Ok(TestPlan::new(suites, &services, &stacks, &changed))
  }
}

/// The repository paths `worktree` changed since `base`, committed or not, and the files it added that git does not
/// ignore; each once, with `/` between its parts.
fn changed_paths(worktree: &Path, base: &str) -> Result<Vec<String>, crate::worktree::WorktreeError> {
  let git = Git::new(worktree);
  let diffed = git.stdout(&["-c", "core.quotepath=false", "diff", "--name-only", base, "--"])?;
  let added = git.stdout(&[
    "-c",
    "core.quotepath=false",
    "ls-files",
    "--others",
    "--exclude-standard",
  ])?;
  let mut changed: Vec<String> = diffed
    .lines()
    .chain(added.lines())
    .filter(|line| !line.trim().is_empty())
    .map(str::to_owned)
    .collect();
  changed.sort();
  changed.dedup();
  Ok(changed)
}

/// One Test step at work and where its news goes.
struct TestWorker {
  root: std::path::PathBuf,
  invocation: Invocation,
  step: StepKey,
  plan: TestPlan,
  suites: StepSuites,
  services: StepServices,
  events: mpsc::UnboundedSender<WorkerEvent>,
  stops: mpsc::UnboundedReceiver<Stop>,
}

impl TestWorker {
  /// Runs the plan until it is done or the run stops it, then tells the run how it ended.
  async fn work(self) {
    let execution = self.invocation.execution;
    let events = self.events.clone();
    let end = self.follow().await;
    // The run's task outlives its steps; if it is gone, the runner is ending anyway.
    if events.send(WorkerEvent::CommandEnded { execution, end }).is_err() {
      tracing::debug!(%execution, "the run no longer waits for its Test step");
    }
  }

  async fn follow(mut self) -> Result<CommandFinished, StoreError> {
    let stop = Arc::new(AtomicBool::new(false));
    let (plan, suites, services) = (self.plan, self.suites, self.services);
    let flag = Arc::clone(&stop);
    let job = tokio::task::spawn_blocking(move || run_layers(plan, &suites, &services, &flag));
    tokio::pin!(job);
    let mut stopped = None;
    let mut listening = true;
    let ran = loop {
      tokio::select! {
        ran = &mut job => break ran,
        told = self.stops.recv(), if listening => match told {
          Some(Stop::Now(cause)) => {
            stopped.get_or_insert(cause);
            stop.store(true, Ordering::Release);
          }
          // A Test step has no turns and runs no commands of an agent: it runs to its end.
          Some(Stop::AfterTurn(_) | Stop::AfterCommand(_) | Stop::AtApproval | Stop::Withdraw) => {}
          None => listening = false,
        },
      }
    };
    let result = match ran {
      Ok(result) => result,
      Err(error) => {
        tracing::error!(%error, execution = %self.invocation.execution, "the Test step's suites ended without a result");
        None
      }
    };
    finish(&self.root, &self.invocation, self.step, (result, stopped))
  }
}

/// Records how the Test step's invocation ended, and what it means for the step: its result when it ran to its end;
/// none when the run stopped it, or when its suites ended without a result, which fails it as a breakage.
fn finish(
  root: &Path,
  invocation: &Invocation,
  step: StepKey,
  (result, stopped): (Option<TestStepResult>, Option<StopCause>),
) -> Result<CommandFinished, StoreError> {
  let mut db = ProjectDb::open(root)?;
  let now = Timestamp::now();
  let mut ended = invocation.clone();
  ended.ended_at = Some(now);
  let finished = match (stopped, result) {
    (Some(cause), _) => {
      ended.outcome = Some(InvocationOutcome::Terminated);
      CommandFinished {
        stopped: Some(cause),
        failure: None,
        output: None,
      }
    }
    (None, Some(result)) => {
      let failure = result.failure();
      ended.outcome = Some(match failure {
        None => InvocationOutcome::Completed,
        Some(_) => InvocationOutcome::ExitFailed { code: None },
      });
      let at = StepAt {
        run: invocation.run,
        execution: invocation.execution,
        step,
        now,
      };
      CommandFinished {
        stopped: None,
        failure,
        output: Some(result.output(at)),
      }
    }
    (None, None) => {
      ended.outcome = Some(InvocationOutcome::ExitFailed { code: None });
      CommandFinished {
        stopped: None,
        failure: Some(StepFailure::NotStarted),
        output: None,
      }
    }
  };
  db.save_invocation(&ended)?;
  Ok(finished)
}

/// Runs `plan`'s layers in order, every suite of a layer, and stops after the first layer that fails; `None` when
/// `stop` was set before it was done.
fn run_layers(
  plan: TestPlan,
  suites: &StepSuites,
  services: &StepServices,
  stop: &AtomicBool,
) -> Option<TestStepResult> {
  let mut result = TestStepResult {
    reached: Vec::new(),
    not_reached: Vec::new(),
    left_out: plan.left_out().to_vec(),
  };
  for layer in plan.into_layers() {
    if result.failed_layer().is_some() {
      let slugs = layer.suites.into_iter().map(|planned| planned.slug).collect();
      result.not_reached.push((layer.layer, slugs));
      continue;
    }
    let mut outcomes = Vec::with_capacity(layer.suites.len());
    for planned in layer.suites {
      outcomes.push(run_suite(planned, suites, services, stop)?);
    }
    result.reached.push((layer.layer, outcomes));
  }
  Some(result)
}

/// Runs `planned` once, then each test that failed once more; `None` when `stop` was set meanwhile.
fn run_suite(
  planned: PlannedSuite,
  suites: &StepSuites,
  services: &StepServices,
  stop: &AtomicBool,
) -> Option<SuiteOutcome> {
  let stopped = || stop.load(Ordering::Acquire);
  if stopped() {
    return None;
  }
  let PlannedSuite { slug, suite, .. } = planned;
  let layer = suite.layer();
  let first = match suites.test(&slug, &suite, None, Some(services)) {
    Ok(kept) => kept,
    Err(why) => return Some(SuiteOutcome::not_run(slug, layer, why)),
  };
  let mut reruns: Vec<SuiteRun> = Vec::new();
  for filter in rerun_filters(&suite, &first.0) {
    if stopped() {
      return None;
    }
    match suites.test(&slug, &suite, filter.as_ref(), Some(services)) {
      Ok((run, _)) => reruns.push(run),
      // The test stays failed: nothing shows it passed.
      Err(why) => tracing::warn!(suite = slug.as_str(), %why, "a failed test did not run again"),
    }
  }
  Some(SuiteOutcome::settle(slug, layer, first, &reruns))
}
