//! Changing a step's place in the pipeline's flow: what the person does with a card (move it up or down, drop it
//! between two places, run it beside a neighbour, take it out of a parallel group), not which step it waits for.
//! The flow is read as in [`Flow`]; the operations rewrite `depends_on` so the steps around the moved one keep their
//! order and the steps after it wait for it.

use std::collections::{HashMap, HashSet};

use thiserror::Error;

use super::{PipelineEdit, PipelineError, Step, StepKey, flow::Flow};

/// Why a step cannot be moved.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum MoveError {
  #[error("`{0}` is not a step of this pipeline")]
  UnknownStep(StepKey),
  #[error("step `{0}` cannot run in parallel with itself")]
  WithItself(StepKey),
  /// The steps break a rule that leaves no flow to move in, e.g. a cycle; it has to be fixed first.
  #[error("the steps have no order to change until this is fixed: {0}")]
  Broken(PipelineError),
  /// The move would put the step a failure route returns to after the step that returns to it.
  #[error("step `{step}` returns to `{target}` on failure, so `{target}` has to run before it")]
  RetryTargetNotUpstream { step: StepKey, target: StepKey },
  /// The step cannot wait for `dependency`, which waits for it: the two would wait for each other.
  #[error("`{dependency}` waits for `{step}`, so `{step}` cannot wait for it")]
  WaitsForIt { step: StepKey, dependency: StepKey },
}

/// Whether a move is offered for a step, and if not, why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveOffer {
  /// Nothing to do: no room, or the step is already there.
  Unavailable,
  /// Offered, but it would break a rule; the reason is for the person.
  Refused(MoveError),
  Available,
}

impl MoveOffer {
  pub const fn is_available(&self) -> bool {
    matches!(self, Self::Available)
  }

  pub(super) fn of(outcome: Result<Option<Vec<Step>>, MoveError>) -> Self {
    match outcome {
      Ok(Some(_)) => Self::Available,
      Ok(None) => Self::Unavailable,
      Err(error) => Self::Refused(error),
    }
  }
}

/// A neighbour a step may run beside: the steps of the unit before or after it in the flow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParallelChoice {
  /// The neighbouring steps in flow order; one when the neighbour runs alone, more for a parallel group.
  pub with: Vec<StepKey>,
  pub offer: MoveOffer,
}

/// The moves of one step, for its menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepMoves {
  /// [`PipelineEdit::move_up`]; unavailable when nothing runs before the step.
  pub up: MoveOffer,
  /// [`PipelineEdit::move_down`]; unavailable when nothing runs after the step.
  pub down: MoveOffer,
  /// [`PipelineEdit::run_on_its_own`]; unavailable unless the step is in a parallel group.
  pub on_its_own: MoveOffer,
  /// [`PipelineEdit::run_in_parallel_with`] for the neighbour before the step, then the one after it.
  pub parallel_with: Vec<ParallelChoice>,
}

impl PipelineEdit {
  /// Step `key` trades places with the step, or the parallel group, before it; a step of a group leaves the group.
  /// Returns whether anything changed (not when nothing runs before it).
  pub fn move_up(&mut self, key: &StepKey) -> Result<bool, MoveError> {
    self.rearrange(key, |flow| Ok(flow.move_up(key)))
  }

  /// Step `key` trades places with the step, or the parallel group, after it; a step of a group leaves the group.
  /// Returns whether anything changed (not when nothing runs after it).
  pub fn move_down(&mut self, key: &StepKey) -> Result<bool, MoveError> {
    self.rearrange(key, |flow| Ok(flow.move_down(key)))
  }

  /// Step `key` lands between two places of the flow, the drop line of a drag: it runs after the steps before the
  /// place and the steps after it run after the step. The place is just before step `before`, or at the end of the
  /// flow when there is none; dropping it before a step of its own group makes it run before that step. Returns
  /// whether anything changed.
  pub fn move_to(&mut self, key: &StepKey, before: Option<&StepKey>) -> Result<bool, MoveError> {
    self.rearrange(key, |flow| flow.move_before(key, before))
  }

  /// Step `key` runs beside `neighbour` (and its parallel group): it waits for what the neighbour waits for, and the
  /// steps that waited for the neighbour wait for it too. Returns whether anything changed (not when they already
  /// run beside each other).
  pub fn run_in_parallel_with(&mut self, key: &StepKey, neighbour: &StepKey) -> Result<bool, MoveError> {
    self.rearrange(key, |flow| flow.run_in_parallel_with(key, neighbour))
  }

  /// Step `key` leaves its parallel group and runs right after it. Returns whether anything changed (not when it
  /// runs alone).
  pub fn run_on_its_own(&mut self, key: &StepKey) -> Result<bool, MoveError> {
    self.rearrange(key, |flow| Ok(flow.run_on_its_own(key)))
  }

  /// Copies step `key` to run right after it, in series: the copy waits for the step, and what waited for the step
  /// waits for the copy. A step of a parallel group is copied after the whole group, the only place after it one line
  /// shows. Steps with no flow to place it in (a cycle) are rewired by the same rule on their dependencies alone.
  /// Returns the copy's key; `None` when there is no such step.
  pub fn duplicate_after(&mut self, key: &StepKey) -> Option<StepKey> {
    let copy = self.duplicate_step(key)?;
    let dependents: Vec<StepKey> = self
      .steps
      .iter()
      .filter(|step| step.key != copy && step.depends_on.contains(key))
      .map(|step| step.key.clone())
      .collect();
    // The copy joins the step's group (it waits for what the step waits for, and the steps after the step wait for it
    // too), then leaves it to run right after.
    for dependent in &dependents {
      self.set_dependency(dependent, &copy, true);
    }
    // Refused only when there is no flow (`MoveError::Broken`): no failure route can refuse it, since the copy returns
    // where the step does, which runs before both, and nothing returns to the copy. The dependencies say it then.
    if self.run_on_its_own(&copy).is_err() {
      self.update(&copy, |step| step.depends_on = std::iter::once(key.clone()).collect());
      for dependent in &dependents {
        self.set_dependency(dependent, key, false);
      }
    }
    Some(copy)
  }

  /// Which moves step `key` has, for its menu. Only the neighbours it can join are listed; a move that would break
  /// a failure route is listed as refused with the reason.
  pub fn possible_moves(&self, key: &StepKey) -> Result<StepMoves, MoveError> {
    let flow = Flow::of(&self.steps).map_err(MoveError::Broken)?;
    let own = flow.unit_of(key).ok_or_else(|| MoveError::UnknownStep(key.clone()))?;
    let offer = |arrange: &dyn Fn(&mut Flow) -> Result<bool, MoveError>| MoveOffer::of(self.rearranged(key, arrange));
    let neighbours = [own.checked_sub(1), Some(own + 1)];
    let parallel_with = neighbours
      .into_iter()
      .flatten()
      .filter_map(|index| flow.unit(index))
      .map(|unit| ParallelChoice {
        with: unit.to_vec(),
        offer: offer(&|flow| flow.run_in_parallel_with(key, &unit[0])),
      })
      .collect();
    Ok(StepMoves {
      up: offer(&|flow| Ok(flow.move_up(key))),
      down: offer(&|flow| Ok(flow.move_down(key))),
      on_its_own: offer(&|flow| Ok(flow.run_on_its_own(key))),
      parallel_with,
    })
  }

  fn rearrange(
    &mut self,
    key: &StepKey,
    arrange: impl FnOnce(&mut Flow) -> Result<bool, MoveError>,
  ) -> Result<bool, MoveError> {
    match self.rearranged(key, arrange)? {
      Some(steps) => {
        self.steps = steps;
        Ok(true)
      }
      None => Ok(false),
    }
  }

  /// The steps after `arrange` changed the flow, or none when it changed nothing.
  fn rearranged(
    &self,
    key: &StepKey,
    arrange: impl FnOnce(&mut Flow) -> Result<bool, MoveError>,
  ) -> Result<Option<Vec<Step>>, MoveError> {
    let before = Flow::of(&self.steps).map_err(MoveError::Broken)?;
    if before.unit_of(key).is_none() {
      return Err(MoveError::UnknownStep(key.clone()));
    }
    let mut flow = before.clone();
    if !arrange(&mut flow)? {
      return Ok(None);
    }
    let steps = flow.arrange(&before, &self.steps);
    if steps == self.steps {
      return Ok(None);
    }
    let broken_before = broken_routes(&self.steps);
    match broken_routes(&steps)
      .into_iter()
      .find(|route| !broken_before.contains(route))
    {
      Some((step, target)) => Err(MoveError::RetryTargetNotUpstream { step, target }),
      None => Ok(Some(steps)),
    }
  }
}

/// The failure routes (step, target) whose target is a step of the pipeline that does not run before the step.
pub(super) fn broken_routes(steps: &[Step]) -> Vec<(StepKey, StepKey)> {
  let by_key: HashMap<&StepKey, &Step> = steps.iter().map(|step| (&step.key, step)).collect();
  steps
    .iter()
    .filter_map(|step| {
      let route = step.on_failure.as_ref()?;
      if !by_key.contains_key(&route.return_to) || runs_before(&by_key, &route.return_to, step) {
        return None;
      }
      Some((step.key.clone(), route.return_to.clone()))
    })
    .collect()
}

/// Whether `target` is a direct or transitive dependency of `step`.
fn runs_before(by_key: &HashMap<&StepKey, &Step>, target: &StepKey, step: &Step) -> bool {
  let mut pending: Vec<&StepKey> = step.depends_on.iter().collect();
  let mut seen = HashSet::new();
  while let Some(current) = pending.pop() {
    if current == target {
      return true;
    }
    if seen.insert(current) {
      pending.extend(by_key.get(current).into_iter().flat_map(|step| step.depends_on.iter()));
    }
  }
  false
}
