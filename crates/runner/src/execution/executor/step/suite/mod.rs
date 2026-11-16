//! `run_tests` and `test_output` (`docs/architecture/agent-testing.md`, "Test suite" and "Agent tools"): the runner
//! runs one of the project's published test suites for an invocation, narrowed by a filter. The services or stacks the
//! suite needs start first, through the invocation's services ([`super::service`]), and those it started stop after it;
//! the program runs in the worktree without a shell ([`program`]); its report is read into the shared shape
//! ([`orchester_core::test_report`]); the files it left are copied into the run's storage ([`artifacts`]); and the
//! result is kept there ([`store`]), so the agent gets the totals and each failure, never the raw output, and reads one
//! test's output with `test_output` when it needs it.
//!
//! A suite has no approval rules of its own yet: each `run_tests` call asks as a service start does.

mod artifacts;
mod program;
mod store;

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
  sync::Arc,
  time::Instant,
};

use orchester_core::{
  connection::stream::RunnerProblem,
  id::ProjectId,
  orchester_tools::ToolResult,
  resource::{CommandEnvironment, ResourceKind, Slug, TestFilter, TestSuite},
  store::ResourceStore,
  test_report::{SuiteRun, TestRunId},
};
use orchester_store::{ProjectDb, StoreError};

use self::{artifacts::Copies, store::RunFolder};
use super::service::StepServices;
use crate::{
  computer::ComputerResources,
  execution::executor::{run::RunTask, step::start::problem},
};

/// The test tools of one invocation.
pub(in crate::execution::executor::step) struct StepSuites {
  /// The project's repository, whose database holds the published suites and the secrets' names.
  root: PathBuf,
  worktree: PathBuf,
  /// The run's own storage, where the results and artifacts are kept (ORC-114).
  storage: PathBuf,
  /// The project's id and the computer the run executes on, whose keychain holds the secrets' values.
  project_id: ProjectId,
  computer: Arc<dyn ComputerResources>,
  /// The environment the runner runs in, whose agent part every suite gets, and what the project adds.
  inherited: Vec<(OsString, OsString)>,
  project: CommandEnvironment,
}

/// The latest published version of each of the project's test suites at `root`, with its slug, removed ones left out.
pub(in crate::execution::executor::step) fn published(root: &Path) -> Result<Vec<(Slug, TestSuite)>, StoreError> {
  let db = ProjectDb::open_read_only(root)?;
  let mut suites = Vec::new();
  for resource in db.resources(ResourceKind::TestSuite)? {
    if let Some(version) = db.latest_version::<TestSuite>(resource.id())? {
      suites.push((resource.slug().clone(), version.body().clone()));
    }
  }
  Ok(suites)
}

/// A suite's run kept with the run, and where its result is.
pub(in crate::execution::executor::step) type Kept = (SuiteRun, String);

impl StepSuites {
  /// Runs the published suite `slug` with `filter`, its needs started through `services` before and those it started
  /// stopped after; the answer of `run_tests`.
  pub(in crate::execution::executor::step) fn run(
    &self,
    slug: &Slug,
    filter: Option<&TestFilter>,
    services: Option<&StepServices>,
  ) -> ToolResult {
    let name = slug.as_str();
    let suite = match published(&self.root) {
      Ok(suites) => suites
        .into_iter()
        .find(|(published, _)| published == slug)
        .map(|(_, suite)| suite),
      Err(error) => return ToolResult::unavailable(&format!("the project's test suites cannot be read: {error}")),
    };
    let Some(suite) = suite else {
      return ToolResult::unavailable(&format!("the project publishes no test suite named `{name}`"));
    };
    if filter.is_some() && !suite.command().accepts_filter() {
      return ToolResult::unavailable(&format!(
        "the suite `{name}` takes no filter: none of its arguments holds {{{{filter}}}}; call run_tests without one"
      ));
    }
    match self.test(slug, &suite, filter, services) {
      Ok((run, stored)) => run.answer(&stored),
      Err(why) => ToolResult::unavailable(&why),
    }
  }

  /// Runs `suite`, published as `slug`, with `filter`, its needs started through `services` before and those it
  /// started stopped after, and keeps its run with the run; why it did not run when it did not. Its own process holds
  /// one of the computer's service slots while it runs, taken through `services` after its needs started.
  pub(in crate::execution::executor::step) fn test(
    &self,
    slug: &Slug,
    suite: &TestSuite,
    filter: Option<&TestFilter>,
    services: Option<&StepServices>,
  ) -> Result<Kept, String> {
    let name = slug.as_str();
    let Some(services) = services else {
      return Err(format!(
        "the suite `{name}` did not run: this step cannot take a service slot for it"
      ));
    };
    let needs = suite.needs().as_slice();
    let started = if needs.is_empty() {
      Vec::new()
    } else {
      services
        .start_needs(needs)
        .map_err(|why| format!("the suite `{name}` did not run: {why}"))?
    };
    let kept = services
      .suite_slot()
      .map_err(|why| format!("the suite `{name}` did not run: {why}"))
      .and_then(|slot| {
        let kept = self.run_started(slug, suite, filter);
        drop(slot);
        kept
      });
    services.stop_started(&started);
    kept
  }

  /// Runs `suite`, whose needs run, and keeps its result with the run.
  fn run_started(&self, slug: &Slug, suite: &TestSuite, filter: Option<&TestFilter>) -> Result<Kept, String> {
    let folder = RunFolder::create(&self.storage)?;
    let began = Instant::now();
    let finished = match self.execute(suite, filter) {
      Ok(finished) => finished,
      Err(why) => {
        folder.discard();
        return Err(format!("the suite `{}` did not run: {why}", slug.as_str()));
      }
    };
    let duration_ms = u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX);
    let mut run = SuiteRun {
      id: folder.id(),
      suite: slug.as_str().to_owned(),
      filter: filter.map(|filter| filter.as_str().to_owned()),
      exit_code: finished.exit_code,
      duration_ms,
      end: finished.end,
      artifacts: Vec::new(),
      artifact_problems: Vec::new(),
    };
    let copies = Copies::of(suite, &self.worktree, run.report(), folder.artifacts());
    run.artifacts = copies.stored;
    run.artifact_problems = copies.problems;
    match folder.save(&run) {
      Ok(stored) => Ok((run, stored)),
      Err(why) => Err(format!("the suite `{}` ran, but {why}", slug.as_str())),
    }
  }

  /// The captured output of the test `test` of the run's test run `run`; the answer of `test_output`.
  pub(in crate::execution::executor::step) fn output(&self, run: TestRunId, test: &str) -> ToolResult {
    match store::load(&self.storage, run) {
      Ok(stored) => stored.test_output(test),
      Err(why) => ToolResult::unavailable(&why),
    }
  }
}

impl RunTask {
  /// The slugs of the project's published test suites, which `run_tests` takes.
  pub(in crate::execution::executor::step) fn suite_slugs(&self) -> Result<Vec<Slug>, RunnerProblem> {
    let suites =
      published(&self.root).map_err(|error| problem(format!("cannot read the project's test suites: {error}")))?;
    Ok(suites.into_iter().map(|(slug, _)| slug).collect())
  }

  /// The test tools of an invocation whose agent works in `worktree` with the project's `environment`.
  pub(in crate::execution::executor::step) fn step_suites(
    &self,
    worktree: &Path,
    environment: &CommandEnvironment,
  ) -> StepSuites {
    StepSuites {
      root: self.root.clone(),
      worktree: worktree.to_path_buf(),
      storage: orchester_core::workspaces::artifacts_folder(&self.root, self.run.id()),
      project_id: self.project.id(),
      computer: Arc::clone(&self.computer),
      inherited: std::env::vars_os().collect(),
      project: environment.clone(),
    }
  }
}
