//! What a requested pause waits for (`docs/design/run-pause.md`): the work of the run's steps that a pause lets end
//! first. A pause never ends an agent's turn or a command by itself; it only stops what would start next, and ends an
//! agent that waits for a person's decision or answer at once. So what remains is the steps that work: an agent
//! finishes its invocation, a command tool's program runs to its end (or its timeout).

use std::sync::Arc;

use orchester_core::run::{RunStatus, StepStatus};

use super::detail::{RunView, StepView};

/// One step the requested pause waits for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PauseWait {
  /// An agent at work ends its invocation first.
  Agent { step: Arc<str>, agent: Option<Arc<str>> },
  /// A command tool's program runs to its end first.
  Command { step: Arc<str>, tool: Option<Arc<str>> },
}

impl PauseWait {
  fn of(step: &StepView) -> Self {
    let step_name = step.name.clone();
    if step.tool_step {
      Self::Command {
        step: step_name,
        tool: step.agent.clone(),
      }
    } else {
      Self::Agent {
        step: step_name,
        agent: step.agent.clone(),
      }
    }
  }

  /// The steps a pause requested for `run` waits for, in pipeline order: none unless the run is running with a pause
  /// requested. Empty while the run only waits for a person: the runner then pauses it at once.
  pub fn of_run(run: &RunView) -> Vec<Self> {
    if run.run.status() != &RunStatus::Running || !run.run.pause_requested() {
      return Vec::new();
    }
    run
      .steps
      .iter()
      .filter(|step| step.status == Some(StepStatus::Running))
      .map(Self::of)
      .collect()
  }
}
