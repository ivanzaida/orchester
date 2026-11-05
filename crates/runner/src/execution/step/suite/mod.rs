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
  orchester_tools::ToolResult,
  resource::{CommandEnvironment, ResourceKind, Slug, TestFilter, TestSuite},
  secret::SecretVault,
  store::ResourceStore,
  test_report::{SuiteRun, TestRunId},
};
use orchester_store::{ProjectDb, StoreError};

use self::{artifacts::Copies, store::RunFolder};
use super::service::StepServices;
use crate::execution::{run::RunTask, step::start::problem};

/// The test tools of one invocation.
pub(in crate::execution::step) struct StepSuites {
  /// The project's repository, whose database holds the published suites and the secrets' names.
  root: PathBuf,
  worktree: PathBuf,
  /// The run's own storage, where the results and artifacts are kept (ORC-114).
  storage: PathBuf,
  vault: Arc<dyn SecretVault>,
  /// The environment the runner runs in, whose agent part every suite gets, and what the project adds.
  inherited: Vec<(OsString, OsString)>,
  project: CommandEnvironment,
}

/// The latest published version of each of the project's test suites at `root`, with its slug, removed ones left out.
fn published(root: &Path) -> Result<Vec<(Slug, TestSuite)>, StoreError> {
  let db = ProjectDb::open_read_only(root)?;
  let mut suites = Vec::new();
  for resource in db.resources(ResourceKind::TestSuite)? {
    if let Some(version) = db.latest_version::<TestSuite>(resource.id())? {
      suites.push((resource.slug().clone(), version.body().clone()));
    }
  }
  Ok(suites)
}

impl StepSuites {
  /// Runs the published suite `slug` with `filter`, its needs started through `services` before and those it started
  /// stopped after; the answer of `run_tests`.
  pub(in crate::execution::step) fn run(
    &self,
    slug: &Slug,
    filter: Option<&TestFilter>,
    services: Option<&StepServices>,
  ) -> ToolResult {
    let name = slug.as_str();
    let suite = match published(&self.root) {
      Ok(suites) => suites.into_iter().find(|(published, _)| published == slug).map(|(_, suite)| suite),
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
    let needs = suite.needs().as_slice();
    let started = match (needs.is_empty(), services) {
      (true, _) => Vec::new(),
      (false, Some(services)) => match services.start_needs(needs) {
        Ok(started) => started,
        Err(why) => return ToolResult::unavailable(&format!("the suite `{name}` did not run: {why}")),
      },
      (false, None) => {
        let needs: Vec<String> = needs.iter().map(|need| format!("`{}`", need.as_str())).collect();
        return ToolResult::unavailable(&format!(
          "the suite `{name}` needs {}, but this step cannot start services",
          needs.join(", ")
        ));
      }
    };
    let answer = self.run_started(slug, &suite, filter);
    if let Some(services) = services {
      services.stop_started(&started);
    }
    answer
  }

  /// Runs `suite`, whose needs run, and keeps its result with the run.
  fn run_started(&self, slug: &Slug, suite: &TestSuite, filter: Option<&TestFilter>) -> ToolResult {
    let folder = match RunFolder::create(&self.storage) {
      Ok(folder) => folder,
      Err(why) => return ToolResult::unavailable(&why),
    };
    let began = Instant::now();
    let finished = match self.execute(suite, filter) {
      Ok(finished) => finished,
      Err(why) => {
        folder.discard();
        return ToolResult::unavailable(&format!("the suite `{}` did not run: {why}", slug.as_str()));
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
      Ok(stored) => run.answer(&stored),
      Err(why) => ToolResult::unavailable(&format!("the suite `{}` ran, but {why}", slug.as_str())),
    }
  }

  /// The captured output of the test `test` of the run's test run `run`; the answer of `test_output`.
  pub(in crate::execution::step) fn output(&self, run: TestRunId, test: &str) -> ToolResult {
    match store::load(&self.storage, run) {
      Ok(stored) => stored.test_output(test),
      Err(why) => ToolResult::unavailable(&why),
    }
  }
}

impl RunTask {
  /// The slugs of the project's published test suites, which `run_tests` takes.
  pub(in crate::execution::step) fn suite_slugs(&self) -> Result<Vec<Slug>, RunnerProblem> {
    let suites =
      published(&self.root).map_err(|error| problem(format!("cannot read the project's test suites: {error}")))?;
    Ok(suites.into_iter().map(|(slug, _)| slug).collect())
  }

  /// The test tools of an invocation whose agent works in `worktree` with the project's `environment`.
  pub(in crate::execution::step) fn step_suites(&self, worktree: &Path, environment: &CommandEnvironment) -> StepSuites {
    StepSuites {
      root: self.root.clone(),
      worktree: worktree.to_path_buf(),
      storage: orchester_core::workspaces::artifacts_folder(&self.root, self.run.id()),
      vault: Arc::clone(&self.vault),
      inherited: std::env::vars_os().collect(),
      project: environment.clone(),
    }
  }
}
