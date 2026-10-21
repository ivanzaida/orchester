//! Which steps of a pipeline merge the base branch when it moves (ORC-54, ORC-65): the steps that implement the
//! change. A step's own choice (`StepPermissions::takes_base_moves`) decides; without one, an agent step with
//! repository write does, unless it checks the work of others — a Review or Verify step, by a word of its key, or one
//! whose failure returns the work to a step that writes. Such a step may write build output and QA files, but never
//! merges the base: that would change the revision it checks. A run's notice that its base moved waits for a step
//! that takes it; the runner also needs the step's agent to write in this run.

use super::{Pipeline, RepositoryAccess, Step, StepKey, StepKind};

/// The words of a step key that name a step checking the work of others.
const CHECKING_WORDS: [&str; 2] = ["review", "verify"];

impl Pipeline {
  /// Whether step `key`'s agent is the one told to merge the base branch when it moves.
  pub fn takes_base_moves(&self, key: &StepKey) -> bool {
    takes_base_moves(self.steps(), key)
  }
}

/// Whether step `key` of `steps` takes base moves: its own choice, else [`takes_base_moves_by_default`]. A tool step
/// or a step that is not there never does.
pub fn takes_base_moves(steps: &[Step], key: &StepKey) -> bool {
  let Some(step) = steps.iter().find(|step| &step.key == key) else {
    return false;
  };
  let StepKind::Agent { permissions, .. } = &step.kind else {
    return false;
  };
  permissions
    .takes_base_moves
    .unwrap_or_else(|| takes_base_moves_by_default(steps, step))
}

/// Whether `step` of `steps` takes base moves when it does not choose: it writes and does not check others' work.
pub fn takes_base_moves_by_default(steps: &[Step], step: &Step) -> bool {
  writes(step) && !checks_work(steps, step)
}

fn writes(step: &Step) -> bool {
  matches!(
    step.kind,
    StepKind::Agent {
      repository: RepositoryAccess::Write,
      ..
    }
  )
}

/// A Review or Verify step by a word of its key (`review`, `design-verify`), or one whose failure returns the work to
/// a step that writes, as Check, Review and Verify return it to Implement.
fn checks_work(steps: &[Step], step: &Step) -> bool {
  let named = step.key.as_str().split('-').any(|word| CHECKING_WORDS.contains(&word));
  let returns_work = step
    .on_failure
    .as_ref()
    .is_some_and(|route| steps.iter().any(|other| other.key == route.return_to && writes(other)));
  named || returns_work
}
