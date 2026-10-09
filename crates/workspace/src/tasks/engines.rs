//! What a native agent's invocation reached its model through, as the runner recorded it before it
//! started (04.H9, 04.H10): the endpoint, who received the prompts and the effort. A step shows what its
//! latest invocation used, not its connection as it is now, once one recorded it.

use orchester_core::{run::Invocation, store::ActivityStore};
use orchester_store::{ProjectDb, StoreError};

use super::detail::StepView;

/// Fills `steps` with the engine their latest invocation among `invocations` recorded, where one did.
pub(super) fn apply_recorded(
  db: &ProjectDb,
  steps: &mut [StepView],
  invocations: &[Invocation],
) -> Result<(), StoreError> {
  for step in steps
    .iter_mut()
    .filter(|step| step.provider.is_some_and(|provider| provider.is_native()))
  {
    let Some(latest) = invocations
      .iter()
      .rev()
      .find(|invocation| Some(invocation.execution) == step.execution)
    else {
      continue;
    };
    if let Some(engine) = db.invocation_engine(latest.id)? {
      step.endpoint = Some(engine.endpoint.as_str().into());
      step.vendor = Some(engine.vendor);
      step.effort = engine.effort;
      step.engine_recorded = true;
    }
  }
  Ok(())
}
