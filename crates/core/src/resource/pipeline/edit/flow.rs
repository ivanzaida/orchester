//! The steps of an edit read as the flow they run in: units that follow each other, the steps of a unit running side
//! by side. A step's unit is its stage (one after the latest stage of the steps it depends on), so a flow can be read
//! from any steps, and rearranging it rewrites only the dependencies the arrangement changes.

use std::collections::{BTreeSet, HashMap, HashSet};

use thiserror::Error;

use super::{MoveError, PipelineError, Step, StepKey};

/// Where a step stood before it left its unit.
struct Detached {
  index: usize,
  /// Whether other steps stay in the unit it left; if not, the unit is gone.
  unit_stays: bool,
}

/// Why steps cannot be drawn as the flow they run in.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum FlowError {
  /// The steps break a rule that leaves no flow, e.g. a cycle.
  #[error("the steps have no flow: {0}")]
  Broken(PipelineError),
  /// A step waits for something other than the whole unit before it, which the flow would draw as if it did: an
  /// independent branch that runs beside other steps, or a step that waits for only some of a parallel group.
  #[error("step `{0}` waits for something other than the steps drawn right before it")]
  Branches(StepKey),
}

/// The steps as the flow they run in, one entry per unit, a unit's steps side by side. Only for steps whose
/// dependencies the flow draws as they are: every step waits for the whole unit before its own (steps listed
/// besides, which that unit already waits for, change nothing), and the steps of the first unit wait for nothing.
pub fn units(steps: &[Step]) -> Result<Vec<Vec<StepKey>>, FlowError> {
  let flow = Flow::of(steps).map_err(FlowError::Broken)?;
  for step in steps {
    let unit = flow
      .unit_of(&step.key)
      .expect("every step of the steps is in their flow");
    let waited_for = flow.waited_for(unit);
    if reduced(steps, &step.depends_on) != waited_for {
      return Err(FlowError::Branches(step.key.clone()));
    }
  }
  Ok(flow.units)
}

/// `dependencies` without those another of them already waits for, directly or through other steps.
fn reduced(steps: &[Step], dependencies: &BTreeSet<StepKey>) -> BTreeSet<StepKey> {
  let by_key: HashMap<&StepKey, &Step> = steps.iter().map(|step| (&step.key, step)).collect();
  let mut implied = HashSet::new();
  for dependency in dependencies {
    let mut pending: Vec<&StepKey> = by_key
      .get(dependency)
      .into_iter()
      .flat_map(|step| step.depends_on.iter())
      .collect();
    while let Some(current) = pending.pop() {
      if implied.insert(current) {
        pending.extend(by_key.get(current).into_iter().flat_map(|step| step.depends_on.iter()));
      }
    }
  }
  dependencies
    .iter()
    .filter(|dependency| !implied.contains(dependency))
    .cloned()
    .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Flow {
  units: Vec<Vec<StepKey>>,
}

impl Flow {
  /// The flow of `steps`, or the rule that keeps them from having one: a repeated key, a dependency on a step that
  /// does not exist, or steps waiting on each other in a circle.
  pub(super) fn of(steps: &[Step]) -> Result<Self, PipelineError> {
    let mut index = HashMap::with_capacity(steps.len());
    for (position, step) in steps.iter().enumerate() {
      if index.insert(&step.key, position).is_some() {
        return Err(PipelineError::DuplicateStep(step.key.clone()));
      }
    }
    for step in steps {
      if let Some(missing) = step.depends_on.iter().find(|key| !index.contains_key(key)) {
        return Err(PipelineError::UnknownDependency {
          step: step.key.clone(),
          dependency: missing.clone(),
        });
      }
    }
    let mut stage: HashMap<&StepKey, usize> = HashMap::with_capacity(steps.len());
    while stage.len() < steps.len() {
      let placed = stage.len();
      for step in steps {
        if stage.contains_key(&step.key) {
          continue;
        }
        let known: Option<Vec<usize>> = step.depends_on.iter().map(|key| stage.get(key).copied()).collect();
        if let Some(known) = known {
          stage.insert(&step.key, known.into_iter().max().map_or(0, |deepest| deepest + 1));
        }
      }
      if stage.len() == placed {
        let stuck = steps
          .iter()
          .find(|step| !stage.contains_key(&step.key))
          .expect("fewer steps are placed than there are, so one is not placed");
        return Err(PipelineError::Cycle(stuck.key.clone()));
      }
    }
    let count = stage.values().max().map_or(0, |deepest| deepest + 1);
    let mut units = vec![Vec::new(); count];
    for step in steps {
      units[stage[&step.key]].push(step.key.clone());
    }
    Ok(Self { units })
  }

  pub(super) fn unit(&self, index: usize) -> Option<&[StepKey]> {
    self.units.get(index).map(Vec::as_slice)
  }

  pub(super) fn unit_of(&self, key: &StepKey) -> Option<usize> {
    self.units.iter().position(|unit| unit.contains(key))
  }

  /// `key` trades places with the unit before it.
  pub(super) fn move_up(&mut self, key: &StepKey) -> bool {
    let Some(index) = self.unit_of(key).filter(|&index| index > 0) else {
      return false;
    };
    self.detach(key);
    self.units.insert(index - 1, vec![key.clone()]);
    true
  }

  /// `key` trades places with the unit after it.
  pub(super) fn move_down(&mut self, key: &StepKey) -> bool {
    let Some(index) = self.unit_of(key).filter(|&index| index + 1 < self.units.len()) else {
      return false;
    };
    let Some(detached) = self.detach(key) else {
      return false;
    };
    self
      .units
      .insert(index + 1 + usize::from(detached.unit_stays), vec![key.clone()]);
    true
  }

  /// `key` becomes a unit of its own right after the group it leaves.
  pub(super) fn run_on_its_own(&mut self, key: &StepKey) -> bool {
    let Some(detached) = self.detach_from_group(key) else {
      return false;
    };
    self.units.insert(detached.index + 1, vec![key.clone()]);
    true
  }

  /// `key` leaves its unit and joins the unit of `neighbour`. Returns whether anything changed.
  pub(super) fn run_in_parallel_with(&mut self, key: &StepKey, neighbour: &StepKey) -> Result<bool, MoveError> {
    if key == neighbour {
      return Err(MoveError::WithItself(key.clone()));
    }
    let theirs = self
      .unit_of(neighbour)
      .ok_or_else(|| MoveError::UnknownStep(neighbour.clone()))?;
    if self.unit_of(key) == Some(theirs) {
      return Ok(false);
    }
    if self.detach(key).is_none() {
      return Ok(false);
    }
    let theirs = self
      .unit_of(neighbour)
      .expect("the neighbour stays in the flow when another step leaves it");
    self.units[theirs].push(key.clone());
    Ok(true)
  }

  /// `key` becomes a unit of its own right before the unit of `before`, or last when there is none.
  pub(super) fn move_before(&mut self, key: &StepKey, before: Option<&StepKey>) -> Result<bool, MoveError> {
    if before == Some(key) {
      return Ok(false);
    }
    if let Some(before) = before {
      self
        .unit_of(before)
        .ok_or_else(|| MoveError::UnknownStep(before.clone()))?;
    }
    if self.detach(key).is_none() {
      return Ok(false);
    }
    let at = before
      .and_then(|before| self.unit_of(before))
      .unwrap_or(self.units.len());
    self.units.insert(at, vec![key.clone()]);
    Ok(true)
  }

  /// `steps` arranged as this flow, which `before` is the arrangement of. A step waits for the whole unit before its
  /// own; steps keep their own dependencies unless the unit before them changed or one of their dependencies is no
  /// longer before them. The steps come out in the flow's order, and a unit's steps in the order they had.
  pub(super) fn arrange(&self, before: &Self, steps: &[Step]) -> Vec<Step> {
    let unit_of: HashMap<&StepKey, usize> = self
      .units
      .iter()
      .enumerate()
      .flat_map(|(index, unit)| unit.iter().map(move |key| (key, index)))
      .collect();
    let mut arranged: Vec<(usize, Step)> = steps
      .iter()
      .map(|step| {
        let unit = unit_of[&step.key];
        let mut step = step.clone();
        let waits_for = self.waited_for(unit);
        let was_waiting_for = before.unit_of(&step.key).map(|old| before.waited_for(old));
        let behind = step.depends_on.iter().any(|dependency| unit_of[dependency] >= unit);
        if behind || was_waiting_for.as_ref() != Some(&waits_for) {
          step.depends_on = waits_for;
        }
        (unit, step)
      })
      .collect();
    arranged.sort_by_key(|(unit, _)| *unit);
    arranged.into_iter().map(|(_, step)| step).collect()
  }

  fn waited_for(&self, unit: usize) -> BTreeSet<StepKey> {
    unit
      .checked_sub(1)
      .and_then(|previous| self.units.get(previous))
      .map(|previous| previous.iter().cloned().collect())
      .unwrap_or_default()
  }

  /// Takes `key` out of its unit; the unit goes with it when it was the only step.
  fn detach(&mut self, key: &StepKey) -> Option<Detached> {
    let index = self.unit_of(key)?;
    self.units[index].retain(|member| member != key);
    let unit_stays = !self.units[index].is_empty();
    if !unit_stays {
      self.units.remove(index);
    }
    Some(Detached { index, unit_stays })
  }

  fn detach_from_group(&mut self, key: &StepKey) -> Option<Detached> {
    self.unit_of(key).filter(|&index| self.units[index].len() > 1)?;
    self.detach(key)
  }
}
