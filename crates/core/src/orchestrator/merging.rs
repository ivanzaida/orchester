//! Whether the Orchestrator turn a task's run success starts by itself merges the run (ORC-69,
//! `docs/design/orchestrator-toolset.md`, decision 5): with *Let the Orchestrator drive* on, a run that passes the
//! chat's rule (`orchester_workspace::orchestrator:: auto_merge`) is merged once in that turn; one that does not is
//! left to the person, the turn telling them why. A step automation accepted as it was counts as passed, as a person's
//! acceptance does (the person's decision of 2026-11-11: full autonomy while the chat drives); the turn that merges
//! such a run tells the person which step it accepted and what it left unverified.

use serde::{Deserialize, Serialize};

use crate::run::{Answerer, Run, RunEvent, RunEventKind, StepOutput, StepStatus, UnverifiedCheck};

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
  /// The step `step` (its name) passed only because automation accepted it as it was. Only waits kept before
  /// 2026-11-11 hold it, when automation's acceptance still left a run to the person; the rule no longer leaves a run
  /// so, and the variant stays so those conversations can still be read and their lines say what they said then.
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
  /// It is a team task's run: it goes into its module's branch and is delivered with the module, which a person
  /// requests.
  TeamDelivery,
  /// It is a team run whose start did not finish recording its team work on this computer, so it can be neither
  /// merged nor delivered from here.
  TeamUnrecorded,
}

impl MergeLeft {
  /// The first step of `run`'s pipeline that has not passed for a merge nobody asked for: one whose latest execution
  /// did not pass. A step accepted as it was counts as passed, whoever accepted it. `None` when every step passed.
  pub fn of_steps(run: &Run) -> Option<Self> {
    run.snapshot().body().steps().iter().find_map(|step| {
      let passed = run
        .latest_execution(&step.key)
        .is_some_and(|latest| latest.status() == StepStatus::Passed);
      (!passed).then(|| Self::StepNotPassed {
        step: step.name.as_str().to_owned(),
      })
    })
  }
}

/// A step of a run that passed because automation accepted it as it was, and what it left unverified: the checks its
/// agent's result could not make, as that result named them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptedUnverified {
  /// The step's name.
  pub step: String,
  pub unverified: Vec<UnverifiedCheck>,
}

impl AcceptedUnverified {
  /// The steps of `run` (its `events` and its steps' `outputs`) that automation accepted as they were, in the order it
  /// accepted them.
  pub fn of_run(run: &Run, events: &[RunEvent], outputs: &[StepOutput]) -> Vec<Self> {
    events
      .iter()
      .filter_map(|event| match event.kind() {
        RunEventKind::StepAccepted {
          execution,
          by: Answerer::Automation,
        } => Some(*execution),
        _ => None,
      })
      .filter_map(|execution| {
        let key = run.execution(execution)?.step();
        let step = run.snapshot().body().step(key)?.name.as_str().to_owned();
        let unverified = outputs
          .iter()
          .find(|output| output.execution == execution)
          .map(|output| output.unverified.clone())
          .unwrap_or_default();
        Some(Self { step, unverified })
      })
      .collect()
  }

  /// What the turn that merges such a run tells the agent about `accepted`; `None` when automation accepted no step.
  pub fn note(accepted: &[Self]) -> Option<String> {
    if accepted.is_empty() {
      return None;
    }
    let mut note = String::from(
      "Steps of this run passed only because you accepted them as they were, unverified. When you tell the person you \
       merged it, name each such step and what it left unverified, plainly, with the link:",
    );
    for step in accepted {
      note.push_str(&format!("\n- {}:", step.step));
      if step.unverified.is_empty() {
        note.push_str(" its result named nothing it could not check.");
      }
      for check in &step.unverified {
        note.push_str(&format!("\n  - {} ({})", check.check, check.reason));
      }
    }
    Some(note)
  }
}
