//! Whether a new version of a pipeline is provably no weaker than the one it replaces: the same steps,
//! wired the same way, run by the same profiles and tools, each as strict or stricter. Anything this cannot
//! prove is treated as weaker, so automation publishes only what a person could not object to; everything
//! else is a person's decision.

use super::{Pipeline, RepositoryAccess, Step, StepKey, StepKind};
use crate::permission::{StepCommands, StepNetwork, StepPermissions};

/// How a new version may be weaker than the one it replaces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Weaker {
  /// Steps were added or removed, so the flow cannot be compared step by step.
  Steps,
  /// The step is wired, run or narrowed differently, or no longer waits or asks where it did.
  Step(StepKey),
}

/// How `new` may be weaker than `old`; empty when it is provably as strict or stricter.
pub fn weaker_than(old: &Pipeline, new: &Pipeline) -> Vec<Weaker> {
  if step_keys(old) != step_keys(new) {
    return vec![Weaker::Steps];
  }
  old
    .steps()
    .iter()
    .filter(|before| new.step(&before.key).is_none_or(|after| !step_no_weaker(before, after)))
    .map(|before| Weaker::Step(before.key.clone()))
    .collect()
}

fn step_keys(pipeline: &Pipeline) -> Vec<&StepKey> {
  let mut keys: Vec<&StepKey> = pipeline.steps().iter().map(|step| &step.key).collect();
  keys.sort();
  keys
}

/// A step is no weaker when it keeps its wiring, failure route (with no more retries before a person is
/// asked), budget (no larger, and kept), instructions, approval and executor, and narrows no less. The
/// retries, the budget and the instructions are where a person checks in, so they count as well.
fn step_no_weaker(old: &Step, new: &Step) -> bool {
  let route = |step: &Step| step.on_failure.as_ref().map(|route| route.return_to.clone());
  let retries = match (&old.on_failure, &new.on_failure) {
    (Some(before), Some(after)) => after.max_retries <= before.max_retries,
    _ => true,
  };
  let budget = match (old.budget, new.budget) {
    (None, _) => true,
    (Some(_), None) => false,
    (Some(before), Some(after)) => after.get() <= before.get(),
  };
  old.depends_on == new.depends_on
    && route(old) == route(new)
    && retries
    && budget
    && old.instructions == new.instructions
    && (!old.approval_gate || new.approval_gate)
    && kind_no_weaker(&old.kind, &new.kind)
}

fn kind_no_weaker(old: &StepKind, new: &StepKind) -> bool {
  match (old, new) {
    (StepKind::Tool { tool: before }, StepKind::Tool { tool: after }) => before == after,
    (
      StepKind::Agent {
        profile: before,
        repository: old_access,
        model: old_model,
        permissions: old_narrowing,
      },
      StepKind::Agent {
        profile: after,
        repository: new_access,
        model: new_model,
        permissions: new_narrowing,
      },
    ) => {
      before == after
        && old_model == new_model
        && (*new_access == RepositoryAccess::ReadOnly || *old_access == RepositoryAccess::Write)
        && narrowing_no_weaker(old_narrowing, new_narrowing)
    }
    _ => false,
  }
}

fn narrowing_no_weaker(old: &StepPermissions, new: &StepPermissions) -> bool {
  let commands = match (&old.commands, &new.commands) {
    (StepCommands::AsProfile, _) => true,
    (StepCommands::Only(_), StepCommands::AsProfile) => false,
    (StepCommands::Only(before), StepCommands::Only(after)) => {
      after.entries().iter().all(|command| before.entries().contains(command))
    }
  };
  commands
    && (old.network == StepNetwork::AsProfile || new.network == StepNetwork::Off)
    && (!old.interactive_tool_approvals || new.interactive_tool_approvals)
}
