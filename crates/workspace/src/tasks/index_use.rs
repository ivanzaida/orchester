//! What each agent launch of a run got of the code index and what it used before its agent first
//! changed a file (`docs/design/orchester-workspace.md`, "Code index", run details), as the runner
//! stored them. Launches recorded before these were kept have neither.

use orchester_core::{
  code_index::CodeIndexUse,
  id::{InvocationId, StepExecutionId},
  run::{EditProgress, Invocation},
  store::ActivityStore,
};
use orchester_store::{ProjectDb, StoreError};

/// One launch's code index and progress to its first edit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaunchIndex {
  pub code_index: Option<CodeIndexUse>,
  pub edit: Option<EditProgress>,
}

/// The launches of a run with what they got, in launch order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IndexUses(Vec<(InvocationId, StepExecutionId, LaunchIndex)>);

impl IndexUses {
  /// Reads what `invocations` got from `db`.
  pub fn read(db: &ProjectDb, invocations: &[Invocation]) -> Result<Self, StoreError> {
    let mut launches = Vec::with_capacity(invocations.len());
    for invocation in invocations {
      let launch = LaunchIndex {
        code_index: db.invocation_code_index(invocation.id)?,
        edit: db.edit_progress(invocation.id)?,
      };
      launches.push((invocation.id, invocation.execution, launch));
    }
    Ok(Self(launches))
  }

  pub fn of(&self, invocation: InvocationId) -> Option<&LaunchIndex> {
    self
      .0
      .iter()
      .find(|(id, ..)| *id == invocation)
      .map(|(.., launch)| launch)
  }

  /// What `execution` used before its first edit over its launches; `None` when a launch has no
  /// stored progress (recorded before it was kept, or a tool step's command).
  pub fn execution_edit(&self, execution: StepExecutionId) -> Option<EditProgress> {
    let parts: Vec<&EditProgress> = self
      .0
      .iter()
      .filter(|(_, of, _)| *of == execution)
      .map(|(.., launch)| launch.edit.as_ref())
      .collect::<Option<_>>()?;
    (!parts.is_empty()).then(|| EditProgress::of_execution(parts))
  }
}
