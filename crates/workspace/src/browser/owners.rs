//! Who owns the run tabs of a project's browser, as the *Browser* window names them (`docs/design/browser.md`, "Tabs"):
//! the run's task key and number (*Run #102 · ORC-107*), the agent profile of the step at work (*Builder is acting in
//! this tab*), and whether the run waits for the person, which the tab's amber dot says.

use std::{collections::HashMap, path::Path, sync::Arc};

use orchester_core::{
  id::RunId,
  run::{Run, RunNumber, RunStatus, StepExecutor, StepStatus},
  store::{ResourceStore, RunStore, TaskStore},
};
use orchester_store::{ProjectDb, RunStoreError};

/// The run a tab belongs to, as the window shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunOwner {
  /// Its task's key (`ORC-107`).
  pub key: Arc<str>,
  pub number: RunNumber,
  /// The name of the agent profile of the step at work or run last; `None` for a tool step or a run not started.
  pub agent: Option<Arc<str>>,
  /// The run waits for the person: a step waits for an approval, a plan decision or a message, or the run stopped
  /// until a person decides.
  pub waits: bool,
}

/// The owners of `runs` among the runs of the project at `root`; a run its database no longer has is left out. Blocks
/// on SQLite.
pub fn run_owners(root: &Path, runs: &[RunId]) -> Result<HashMap<RunId, RunOwner>, RunStoreError> {
  let db = ProjectDb::open(root)?;
  let mut owners = HashMap::new();
  for id in runs {
    let Some(run) = db.run(*id)? else {
      tracing::debug!(run = %id, "a browser tab's run is not in the project's database");
      continue;
    };
    let Some(task) = db.task(run.task())? else {
      continue;
    };
    let agent = match working_profile(&run) {
      Some(profile) => db.resource(profile)?.map(|resource| Arc::from(resource.name().as_str())),
      None => None,
    };
    owners.insert(
      *id,
      RunOwner {
        key: Arc::from(task.key().to_string()),
        number: run.number(),
        agent,
        waits: waits_for_person(&run),
      },
    );
  }
  Ok(owners)
}

/// The agent profile of the step of `run`'s current attempt that works or waits, else of the one that ran last.
fn working_profile(run: &Run) -> Option<orchester_core::id::ResourceId> {
  let executions = run.current_attempt()?.executions();
  let execution = executions
    .iter()
    .rev()
    .find(|execution| matches!(execution.status(), StepStatus::Running | StepStatus::Waiting(_)))
    .or_else(|| executions.last())?;
  match run.snapshot().step(execution.step())?.executor() {
    StepExecutor::Agent { profile, .. } => Some(profile.resource),
    StepExecutor::Tool { .. } => None,
  }
}

fn waits_for_person(run: &Run) -> bool {
  if matches!(run.status(), RunStatus::WaitingForPerson(_)) {
    return true;
  }
  run.current_attempt().is_some_and(|attempt| {
    attempt
      .executions()
      .iter()
      .any(|execution| matches!(execution.status(), StepStatus::Waiting(_)))
  })
}
