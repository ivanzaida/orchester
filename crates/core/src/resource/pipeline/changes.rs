//! What changed between two versions of a pipeline, step by step, for the publish
//! confirmation: added, removed and edited steps, with before and after values where they are
//! short enough to show.

use super::{Pipeline, Step, StepKey, StepKind};
use crate::{money::Money, name::Name};

/// One difference between two versions of a pipeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PipelineChange {
  Description,
  Step {
    key: StepKey,
    name: Name,
    change: StepChange,
  },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepChange {
  Added,
  Removed,
  Renamed {
    from: Name,
  },
  /// A different agent profile or tool, or switched between the two.
  Executor,
  Model,
  RepositoryAccess,
  Instructions,
  Dependencies,
  ApprovalGate {
    now: bool,
  },
  /// The step returns somewhere else when it fails, or no longer returns, or now does.
  FailureRoute,
  MaxRetries {
    from: u32,
    to: u32,
  },
  Budget {
    from: Option<Money>,
    to: Option<Money>,
  },
}

/// Changes from `from` to `to`: the description, then edited and added steps in the order of
/// `to`, then removed steps in the order of `from`.
pub fn changes(from: &Pipeline, to: &Pipeline) -> Vec<PipelineChange> {
  let mut changes = Vec::new();
  if from.description() != to.description() {
    changes.push(PipelineChange::Description);
  }
  for step in to.steps() {
    let found = from.step(&step.key);
    let step_changes = found.map_or_else(|| vec![StepChange::Added], |before| step_changes(before, step));
    changes.extend(step_changes.into_iter().map(|change| PipelineChange::Step {
      key: step.key.clone(),
      name: step.name.clone(),
      change,
    }));
  }
  for step in from.steps().iter().filter(|step| to.step(&step.key).is_none()) {
    changes.push(PipelineChange::Step {
      key: step.key.clone(),
      name: step.name.clone(),
      change: StepChange::Removed,
    });
  }
  changes
}

fn step_changes(before: &Step, after: &Step) -> Vec<StepChange> {
  let mut changes = Vec::new();
  if before.name != after.name {
    changes.push(StepChange::Renamed {
      from: before.name.clone(),
    });
  }
  changes.extend(kind_changes(&before.kind, &after.kind));
  if before.instructions != after.instructions {
    changes.push(StepChange::Instructions);
  }
  if before.depends_on != after.depends_on {
    changes.push(StepChange::Dependencies);
  }
  if before.approval_gate != after.approval_gate {
    changes.push(StepChange::ApprovalGate {
      now: after.approval_gate,
    });
  }
  match (&before.on_failure, &after.on_failure) {
    (Some(was), Some(now)) if was.return_to == now.return_to && was.max_retries != now.max_retries => {
      changes.push(StepChange::MaxRetries {
        from: was.max_retries.get(),
        to: now.max_retries.get(),
      });
    }
    (Some(was), Some(now)) if was.return_to == now.return_to => {}
    (was, now) if was != now => changes.push(StepChange::FailureRoute),
    _ => {}
  }
  if before.budget != after.budget {
    changes.push(StepChange::Budget {
      from: before.budget,
      to: after.budget,
    });
  }
  changes
}

fn kind_changes(before: &StepKind, after: &StepKind) -> Vec<StepChange> {
  match (before, after) {
    (
      StepKind::Agent {
        profile,
        repository,
        model,
      },
      StepKind::Agent {
        profile: new_profile,
        repository: new_repository,
        model: new_model,
      },
    ) => [
      (profile != new_profile, StepChange::Executor),
      (model != new_model, StepChange::Model),
      (repository != new_repository, StepChange::RepositoryAccess),
    ]
    .into_iter()
    .filter_map(|(changed, change)| changed.then_some(change))
    .collect(),
    (StepKind::Tool { tool }, StepKind::Tool { tool: new_tool }) if tool == new_tool => Vec::new(),
    _ => vec![StepChange::Executor],
  }
}
