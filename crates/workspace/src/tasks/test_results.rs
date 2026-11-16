//! What a run's Test step found, as the task page's Tests card shows it (ORC-266, `07 · Summary · Tests` `qN2FK`):
//! the record its latest execution that ran to its end kept in the run's storage
//! ([`orchester_core::test_step::record_file`]), which attempt that was, and the step its failed tests went back to. A
//! run whose pipeline has no Test step, or whose Test step has not finished yet, has none.

use std::{
  fs, io,
  num::NonZeroU32,
  path::{Component, Path},
  sync::Arc,
};

use orchester_core::{
  id::StepExecutionId,
  resource::StepKey,
  run::{AttemptReason, Run, StepExecutor},
  test_step::{TestStepRecord, record_file},
  workspaces::artifacts_folder,
};

/// The latest Test step results of a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestResults {
  pub step: StepKey,
  pub execution: StepExecutionId,
  /// The attempt the execution ran in.
  pub attempt: NonZeroU32,
  /// The record, or why it cannot be read.
  pub record: Result<TestStepRecord, Arc<str>>,
  /// The step the failed tests went back to, when the step's failure route returned them.
  pub returned_to: Option<StepKey>,
}

/// The results of `run`'s latest Test step execution whose record is kept in the repository at `repo_root`.
pub fn read(repo_root: &Path, run: &Run) -> Option<TestResults> {
  let storage = artifacts_folder(repo_root, run.id());
  let attempts = run.attempts();
  for (index, attempt) in attempts.iter().enumerate().rev() {
    for execution in attempt.executions().iter().rev() {
      let tests = run
        .snapshot()
        .step(execution.step())
        .is_some_and(|step| matches!(step.executor(), StepExecutor::Test));
      if !tests {
        continue;
      }
      let Some(record) = load(&storage, execution.id()) else {
        continue;
      };
      let returned_to = attempts.get(index + 1).and_then(|next| match next.reason() {
        AttemptReason::Retry { failed_step, return_to } if failed_step == execution.step() => Some(return_to.clone()),
        _ => None,
      });
      return Some(TestResults {
        step: execution.step().clone(),
        execution: execution.id(),
        attempt: attempt.number(),
        record: record.map(|record| within(record, &storage)),
        returned_to,
      });
    }
  }
  None
}

/// The record of `execution` in `storage`; `None` when it kept none.
fn load(storage: &Path, execution: StepExecutionId) -> Option<Result<TestStepRecord, Arc<str>>> {
  let file = record_file(storage, execution);
  let text = match fs::read(&file) {
    Ok(text) => text,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
    Err(error) => return Some(Err(Arc::from(error.to_string()))),
  };
  Some(serde_json::from_slice(&text).map_err(|error| Arc::from(error.to_string())))
}

/// `record` with only the artifacts kept in the run's `storage`: the page opens no file outside it.
fn within(mut record: TestStepRecord, storage: &Path) -> TestStepRecord {
  for suite in record.layers.iter_mut().flat_map(|layer| layer.suites.iter_mut()) {
    suite.artifacts.retain(|artifact| inside(&artifact.stored, storage));
    for test in suite.failed.iter_mut().chain(suite.quarantined.iter_mut()) {
      test.artifacts.retain(|stored| inside(stored, storage));
    }
  }
  record
}

/// Whether `path` names a file inside `storage`, without a way back out of it.
fn inside(path: &str, storage: &Path) -> bool {
  let path = Path::new(path);
  path.starts_with(storage) && !path.components().any(|part| part == Component::ParentDir)
}
