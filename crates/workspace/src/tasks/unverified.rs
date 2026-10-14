//! What a run waiting for a person without a verdict holds for the task page and the domain tools: what its step
//! could not check, and for one not run again on an unchanged revision, the verdict that held.

use orchester_core::{
  resource::StepKey,
  run::{RunStatus, StepOutcome, StepOutput, Stuck, UnverifiedCheck},
};

use super::detail::RunView;

impl RunView {
  /// What a run waiting unverified or unchanged could not check: the checks its step's latest result listed, empty
  /// for a result that listed none (an unchanged step whose earlier result failed) and for a run that does not wait
  /// that way.
  pub fn unverified(&self) -> &[UnverifiedCheck] {
    let RunStatus::WaitingForPerson(stuck @ (Stuck::Unverified(_) | Stuck::Unchanged(_))) = self.run.status() else {
      return &[];
    };
    self
      .latest_output(stuck.step())
      .map_or(&[], |output| output.unverified.as_slice())
  }

  /// For a run waiting unchanged, the earlier result its step was not run again for, when that was a failed verdict.
  pub fn unchanged_verdict(&self) -> Option<&StepOutput> {
    let RunStatus::WaitingForPerson(Stuck::Unchanged(step)) = self.run.status() else {
      return None;
    };
    self
      .latest_output(step)
      .filter(|output| output.outcome == Some(StepOutcome::Failed))
  }

  /// The latest output `step` left, in any attempt.
  pub fn latest_output(&self, step: &StepKey) -> Option<&StepOutput> {
    self.outputs.iter().rev().find(|output| output.step == *step)
  }
}
