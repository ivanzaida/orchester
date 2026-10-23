//! Whether the Orchestrator turn a task's run success starts by itself merges the run (ORC-69,
//! `docs/design/orchestrator-toolset.md`, decision 5): with *Let the Orchestrator merge finished runs on its own* on,
//! besides *merge runs* and *answer agents*, a run that passes the chat's rule (`orchester_workspace::orchestrator::
//! auto_merge`) is merged once in that turn; one that does not is left to the person, the turn telling them why.

use serde::{Deserialize, Serialize};

use crate::run::{Answerer, AttemptReason, AttemptStart, Run, StepStatus};

/// What the turn of a run's success does about merging the run, while the person lets the Orchestrator merge
/// finished runs on its own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "merging", rename_all = "snake_case")]
pub enum Merging {
  /// It merges the run itself, once, with `merge_run`.
  Merges,
  /// The run is left to the person, for `why`.
  Left { why: MergeLeft },
}

/// Why a run that succeeded is not merged on its own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "left", rename_all = "snake_case")]
pub enum MergeLeft {
  /// It is not its task's latest run that succeeded (any more).
  NotReady,
  /// The step `step` (its name) of its pipeline has not passed.
  StepNotPassed { step: String },
  /// The step `step` (its name) passed only because automation accepted it as it was; a person's acceptance counts.
  AcceptedByAutomation { step: String },
  /// The task is marked Blocked, for `reason`.
  MarkedBlocked { reason: String },
  /// Unfinished tasks block it: `tasks`, their keys.
  BlockedByTasks { tasks: Vec<String> },
  /// New starts are held for its reviewed group or the whole project.
  Held,
  /// The task says *Don't merge automatically*.
  OptedOut,
  /// Its pipeline ends in a Deliver step: the merge queue delivers it, after a person's approval when the step asks
  /// for one (ORC-19).
  DeliveredByQueue,
}

impl MergeLeft {
  /// The first step of `run`'s pipeline that has not passed for a merge nobody asked for: one whose latest execution
  /// did not pass, or that passed only by an acceptance automation decided (`starts`: the run's attempt starts,
  /// [`crate::run::attempt_starts`]). A step a person accepted as it was counts as passed. `None` when every step
  /// passed so.
  pub fn of_steps(run: &Run, starts: &[AttemptStart]) -> Option<Self> {
    for step in run.snapshot().body().steps() {
      let name = || step.name.as_str().to_owned();
      let Some(latest) = run
        .latest_execution(&step.key)
        .filter(|latest| latest.status() == StepStatus::Passed)
      else {
        return Some(Self::StepNotPassed { step: name() });
      };
      let accepted_in = run.attempts().iter().find(|attempt| {
        attempt.execution(latest.id()).is_some()
          && matches!(attempt.reason(), AttemptReason::Accepted { step: accepted } if *accepted == step.key)
      });
      let Some(attempt) = accepted_in else {
        continue;
      };
      let by = starts
        .iter()
        .find(|start| start.attempt == attempt.id())
        .and_then(|start| start.by);
      if by != Some(Answerer::Person) {
        return Some(Self::AcceptedByAutomation { step: name() });
      }
    }
    None
  }
}
