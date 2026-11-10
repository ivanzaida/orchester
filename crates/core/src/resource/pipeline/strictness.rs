//! Whether a new version of a pipeline is provably no weaker than the one it replaces: the same steps,
//! wired so that every step still waits for the approval-gated and tool steps it waited for, run by the same
//! profiles and tools, each as strict or stricter. Anything this cannot
//! prove is treated as weaker, so automation publishes only what a person could not object to; everything
//! else is a person's decision.

use std::collections::HashSet;

use super::{Pipeline, RepositoryAccess, Step, StepKey, StepKind};
use crate::permission::{StepCommands, StepNetwork, StepPermissions};

/// How a new version may be weaker than the one it replaces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Weaker {
  /// Steps were added or removed, so the flow cannot be compared step by step.
  Steps,
  /// The sources the pipeline or its steps list changed: what its agents read is a person's to choose.
  Sources,
  /// The step is run or narrowed differently, or no longer asks where it did.
  Step(StepKey),
  /// The step no longer waits, directly or through other steps, for these approval-gated or tool steps it waited for.
  StopsWaiting { step: StepKey, waited_for: Vec<StepKey> },
}

/// How `new` may be weaker than `old`; empty when it is provably as strict or stricter.
pub fn weaker_than(old: &Pipeline, new: &Pipeline) -> Vec<Weaker> {
  if step_keys(old) != step_keys(new) {
    return vec![Weaker::Steps];
  }
  let sources = (old.sources() != new.sources()).then_some(Weaker::Sources);
  let mut weaker: Vec<Weaker> = sources.into_iter().collect();
  for before in old.steps() {
    let Some(after) = new.step(&before.key) else {
      weaker.push(Weaker::Step(before.key.clone()));
      continue;
    };
    let waited_for = stops_waiting_for(old, new, &before.key);
    if !waited_for.is_empty() {
      weaker.push(Weaker::StopsWaiting {
        step: before.key.clone(),
        waited_for,
      });
    }
    if !step_no_weaker(before, after) {
      weaker.push(Weaker::Step(before.key.clone()));
    }
  }
  weaker
}

fn step_keys(pipeline: &Pipeline) -> Vec<&StepKey> {
  let mut keys: Vec<&StepKey> = pipeline.steps().iter().map(|step| &step.key).collect();
  keys.sort();
  keys
}

/// A step is no weaker when it keeps its failure route (with no more retries before a person is
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
  route(old) == route(new)
    && retries
    && budget
    && old.instructions == new.instructions
    && (!old.approval_gate || new.approval_gate)
    && kind_no_weaker(&old.kind, &new.kind)
}

/// The approval-gated steps and tool steps step `key` waited for in `old`, directly or through other steps, that it no
/// longer waits for in `new`, in the order of `old`'s steps. Waiting for other steps differently is a reorder: it
/// changes when the step runs, not what stands between it and the run's end.
fn stops_waiting_for(old: &Pipeline, new: &Pipeline, key: &StepKey) -> Vec<StepKey> {
  let (before, after) = (ancestors(old, key), ancestors(new, key));
  old
    .steps()
    .iter()
    .filter(|step| checks(step) && before.contains(&step.key) && !after.contains(&step.key))
    .map(|step| step.key.clone())
    .collect()
}

/// A step the run waits at: it waits for a person's approval, or it runs a tool.
fn checks(step: &Step) -> bool {
  step.approval_gate || matches!(step.kind, StepKind::Tool { .. })
}

/// The steps `key` waits for, directly or through others.
fn ancestors<'a>(pipeline: &'a Pipeline, key: &StepKey) -> HashSet<&'a StepKey> {
  let mut found = HashSet::new();
  let mut pending: Vec<&StepKey> = pipeline
    .step(key)
    .into_iter()
    .flat_map(|step| step.depends_on.iter())
    .collect();
  while let Some(next) = pending.pop() {
    if found.insert(next) {
      pending.extend(pipeline.step(next).into_iter().flat_map(|step| step.depends_on.iter()));
    }
  }
  found
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
  // Which step merges the base is a person's choice either way: it is not proven no weaker unless it is kept.
  commands
    && (old.network == StepNetwork::AsProfile || new.network == StepNetwork::Off)
    && (!old.interactive_tool_approvals || new.interactive_tool_approvals)
    && old.takes_base_moves == new.takes_base_moves
}
