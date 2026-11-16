//! What each step waits for, which the flow draws by stage: a step stands one stage below the latest step it waits for,
//! so one that waits for only some of the stage drawn before it says which ([`part_waits`]). A step's *Waits for*
//! changes what it waits for one step at a time ([`PipelineEdit::wait_for`]) and offers only the steps that do not wait
//! for it, so no change made there leaves the steps waiting for each other. Steps that have no order to draw are given
//! one: each waits for the step listed before it ([`PipelineEdit::make_line`]).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::{FlowError, MoveError, MoveOffer, PipelineEdit, Step, StepKey, flow::Flow, moves::broken_routes};

/// A step that waits for less than the whole stage drawn before it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartWait {
  /// What it waits for, without what one of them already waits for, in flow order.
  pub on: Vec<StepKey>,
  /// The steps of the stage before it that it does not wait for, even through others, in flow order: it may start
  /// before they end.
  pub not_on: Vec<StepKey>,
}

/// Whether a step waits for another, as its *Waits for* says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Waited {
  /// It names the step: one of its chips.
  Directly,
  /// It waits for the step through this one, which it names, so naming the step too changes nothing.
  Through(StepKey),
  No,
}

/// A step that another may wait for, and what ticking or unticking it would do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitChoice {
  pub key: StepKey,
  pub waited: Waited,
  /// Available when ticking a step it does not wait for, or unticking one it names, changes the steps; refused with
  /// why when it would break a failure route; unavailable for a step it waits for through another.
  pub offer: MoveOffer,
}

/// The steps of `steps` that wait for only part of the stage drawn before them, by key.
pub fn part_waits(steps: &[Step]) -> Result<BTreeMap<StepKey, PartWait>, FlowError> {
  let flow = Flow::of(steps).map_err(FlowError::Broken)?;
  let graph = Graph::of(steps);
  let order = order(&flow);
  let mut waits = BTreeMap::new();
  for step in steps {
    let unit = flow
      .unit_of(&step.key)
      .expect("every step of the steps is in their flow");
    let Some(before) = unit.checked_sub(1).and_then(|previous| flow.unit(previous)) else {
      continue;
    };
    let upstream = graph.upstream(&step.key);
    let not_on: Vec<StepKey> = before.iter().filter(|key| !upstream.contains(key)).cloned().collect();
    if not_on.is_empty() {
      continue;
    }
    let mut on: Vec<StepKey> = graph.reduced(&step.depends_on).into_iter().collect();
    on.sort_by_key(|key| order.get(key).copied());
    waits.insert(step.key.clone(), PartWait { on, not_on });
  }
  Ok(waits)
}

impl PipelineEdit {
  /// The steps step `key` may wait for, in flow order: every other step that does not wait for it, directly or through
  /// others, so none of them makes the two wait for each other.
  pub fn wait_choices(&self, key: &StepKey) -> Result<Vec<WaitChoice>, MoveError> {
    let flow = Flow::of(&self.steps).map_err(MoveError::Broken)?;
    let step = self.step(key).ok_or_else(|| MoveError::UnknownStep(key.clone()))?;
    let graph = Graph::of(&self.steps);
    let named = graph.reduced(&step.depends_on);
    let choices = flow
      .order()
      .filter(|other| *other != key && !graph.upstream(other).contains(key))
      .map(|other| {
        let through = flow
          .order()
          .filter(|by| named.contains(*by))
          .find(|by| graph.upstream(by).contains(other));
        let (waited, offer) = if named.contains(other) {
          (Waited::Directly, MoveOffer::of(self.waiting(key, other, false)))
        } else if let Some(by) = through {
          (Waited::Through(by.clone()), MoveOffer::Unavailable)
        } else {
          (Waited::No, MoveOffer::of(self.waiting(key, other, true)))
        };
        WaitChoice {
          key: other.clone(),
          waited,
          offer,
        }
      })
      .collect();
    Ok(choices)
  }

  /// Makes step `key` wait for `dependency` (`waits`) or no longer name it. What it names is kept without what one of
  /// them already waits for. Returns whether anything changed: not when it already waits for `dependency` (or, for
  /// `false`, does not name it). Refused when `dependency` waits for the step, or when the change leaves a failure
  /// route returning to a step that no longer runs before.
  pub fn wait_for(&mut self, key: &StepKey, dependency: &StepKey, waits: bool) -> Result<bool, MoveError> {
    Flow::of(&self.steps).map_err(MoveError::Broken)?;
    Ok(self.waiting(key, dependency, waits)?.is_some_and(|steps| {
      self.steps = steps;
      true
    }))
  }

  /// Makes step `key` wait for the whole stage drawn before it as well as what it waits for, which puts it back under
  /// that stage. Returns whether anything changed: not for a step of the first stage, or one that waits for all of it.
  pub fn wait_for_stage_before(&mut self, key: &StepKey) -> Result<bool, MoveError> {
    let flow = Flow::of(&self.steps).map_err(MoveError::Broken)?;
    let unit = flow.unit_of(key).ok_or_else(|| MoveError::UnknownStep(key.clone()))?;
    let Some(before) = unit.checked_sub(1).and_then(|previous| flow.unit(previous)) else {
      return Ok(false);
    };
    let graph = Graph::of(&self.steps);
    let upstream = graph.upstream(key);
    if before.iter().all(|other| upstream.contains(other)) {
      return Ok(false);
    }
    let step = self.step(key).expect("a step of the flow is a step of the edit");
    let wanted = graph.reduced(&step.depends_on.iter().chain(before).cloned().collect());
    Ok(self.update(key, |step| step.depends_on = wanted))
  }

  /// Gives the steps one line: each waits for the step listed before it, the first for nothing, replacing what every
  /// step waited for. Returns whether anything changed.
  pub fn make_line(&mut self) -> bool {
    let mut changed = false;
    let mut previous: Option<StepKey> = None;
    for step in &mut self.steps {
      let wanted: BTreeSet<StepKey> = previous.iter().cloned().collect();
      if step.depends_on != wanted {
        step.depends_on = wanted;
        changed = true;
      }
      previous = Some(step.key.clone());
    }
    changed
  }

  /// Whether [`Self::make_line`] would leave steps that break no rule, so they have an order to draw.
  pub fn line_fixes(&self) -> bool {
    let mut line = self.clone();
    line.make_line();
    line.validate().is_ok()
  }

  /// The steps once step `key` waits for `dependency` or no longer names it; `None` when that changes nothing.
  fn waiting(&self, key: &StepKey, dependency: &StepKey, waits: bool) -> Result<Option<Vec<Step>>, MoveError> {
    let step = self.step(key).ok_or_else(|| MoveError::UnknownStep(key.clone()))?;
    if self.step(dependency).is_none() {
      return Err(MoveError::UnknownStep(dependency.clone()));
    }
    let graph = Graph::of(&self.steps);
    if waits && (dependency == key || graph.upstream(dependency).contains(key)) {
      return Err(MoveError::WaitsForIt {
        step: key.clone(),
        dependency: dependency.clone(),
      });
    }
    let named = graph.reduced(&step.depends_on);
    let mut wanted = named.clone();
    if waits {
      wanted.insert(dependency.clone());
      wanted = graph.reduced(&wanted);
    } else {
      wanted.remove(dependency);
    }
    if wanted == named {
      return Ok(None);
    }
    let mut steps = self.steps.clone();
    if let Some(changed) = steps.iter_mut().find(|step| &step.key == key) {
      changed.depends_on = wanted;
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

/// The position of every step of `flow` in flow order.
fn order(flow: &Flow) -> HashMap<&StepKey, usize> {
  flow.order().enumerate().map(|(index, key)| (key, index)).collect()
}

/// The steps by key, to follow what they wait for.
struct Graph<'a> {
  steps: HashMap<&'a StepKey, &'a Step>,
}

impl<'a> Graph<'a> {
  fn of(steps: &'a [Step]) -> Self {
    Self {
      steps: steps.iter().map(|step| (&step.key, step)).collect(),
    }
  }

  /// Every step `key` waits for, directly or through others.
  fn upstream(&self, key: &StepKey) -> HashSet<&'a StepKey> {
    self
      .steps
      .get(key)
      .map(|step| self.upstream_of(&step.depends_on))
      .unwrap_or_default()
  }

  /// `dependencies` and every step they wait for, directly or through others.
  fn upstream_of(&self, dependencies: &BTreeSet<StepKey>) -> HashSet<&'a StepKey> {
    let mut seen = HashSet::new();
    let mut pending: Vec<&StepKey> = dependencies.iter().collect();
    while let Some(current) = pending.pop() {
      let Some((&key, step)) = self.steps.get_key_value(current) else {
        continue;
      };
      if seen.insert(key) {
        pending.extend(step.depends_on.iter());
      }
    }
    seen
  }

  /// `dependencies` without those another of them already waits for, directly or through others.
  fn reduced(&self, dependencies: &BTreeSet<StepKey>) -> BTreeSet<StepKey> {
    let implied: HashSet<&StepKey> = dependencies
      .iter()
      .flat_map(|dependency| self.upstream(dependency))
      .collect();
    dependencies
      .iter()
      .filter(|dependency| !implied.contains(dependency))
      .cloned()
      .collect()
  }
}
