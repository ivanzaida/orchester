//! Check on the merged result: the command tools of the run's checking tool steps, as the run pinned them, run again in
//! the run's worktree once it holds the current base branch, in pipeline order. Every tool step checks but the
//! starter's *Create pull request*, which delivers the work rather than checks it, and never runs again
//! ([`rechecks`]). A pipeline without a checking command tool step has nothing to run again.
//!
//! The desktop only names the tools ([`recheck_tools`]); the project's runner runs them in the run's worktree, as its
//! tool steps run them (`orchester_core::merge_queue::recheck`, ORC-684).

use orchester_core::{
  merge_queue::recheck::RecheckTool,
  resource::{StarterStep, StepKey, Tool, ToolAction},
  run::{Run, StepExecutor},
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};

/// Whether the tool step keyed `key` runs again on the merged result: every one but the starter's *Create pull
/// request*, which opens a pull request rather than checks the work.
pub fn rechecks(key: &StepKey) -> bool {
  key.as_str() != StarterStep::CreatePullRequest.key()
}

/// The command tools of `run`'s checking tool steps, in pipeline order, as the run pinned them in `db`; a tool named
/// by its step's key when the tool is gone.
pub(super) fn recheck_tools(db: &ProjectDb, run: &Run) -> Result<Vec<RecheckTool>, StoreError> {
  let mut tools = Vec::new();
  for step in run.snapshot().steps().iter().filter(|step| rechecks(step.key())) {
    let StepExecutor::Tool { tool: pinned } = step.executor() else {
      continue;
    };
    let Some(version) = db.version::<Tool>(pinned.resource, pinned.number)? else {
      continue;
    };
    let ToolAction::Command(action) = version.body().action() else {
      continue;
    };
    let name = db.resource(pinned.resource)?.map_or_else(
      || step.key().to_string(),
      |resource| resource.name().as_str().to_owned(),
    );
    tools.push(RecheckTool {
      name,
      action: action.clone(),
    });
  }
  Ok(tools)
}
