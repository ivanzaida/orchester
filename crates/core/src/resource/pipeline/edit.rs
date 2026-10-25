//! Editing the steps of a pipeline draft. Edits may break pipeline rules on the way — a step
//! returning to a removed one, a cycle, no steps at all — so the edited steps are kept as they
//! are and [`PipelineEdit::validate`] says which rule is broken. Only a valid list becomes a
//! [`Pipeline`] that can be saved.

use std::collections::BTreeSet;

use super::{
  DeliverStep, Deliverable, Pipeline, PipelineError, PipelineSources, RepositoryAccess, Step, StepKey, StepKind,
  unassigned,
};
use crate::{name::Name, permission::StepPermissions, resource::prompt::PromptText};

/// Base of the keys given to added steps: `step`, `step-2`, …
const ADDED_KEY: &str = "step";
/// Suffix of the key of a duplicated step: `review-copy`, `review-copy-2`, …
const COPY_SUFFIX: &str = "copy";

/// Something that keeps the edited steps from being published.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditProblem {
  NoSteps,
  /// Steps waiting on each other in a circle, in flow order: the first step waits for the
  /// last one, which closes the circle, e.g. `[plan, implement, review]` when Plan waits for
  /// Review.
  Cycle(Vec<StepKey>),
  /// Any other pipeline rule the steps break.
  Rule(PipelineError),
  /// A step without an agent profile or tool.
  Unassigned(StepKey),
}

impl EditProblem {
  /// Whether the steps cannot even be kept as a draft while this problem lasts.
  pub const fn blocks_saving(&self) -> bool {
    matches!(self, Self::Cycle(_) | Self::Rule(_))
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineEdit {
  description: String,
  /// The pipeline's sources; a removed step's go with it.
  sources: PipelineSources,
  steps: Vec<Step>,
  /// The Deliver step the edited pipeline ends in, if any.
  deliver: Option<DeliverStep>,
  /// What the edited pipeline's runs deliver.
  delivers: Deliverable,
}

impl PipelineEdit {
  pub fn new(pipeline: Pipeline) -> Self {
    Self {
      description: pipeline.description().to_owned(),
      sources: pipeline.sources().clone(),
      deliver: pipeline.deliver().cloned(),
      delivers: pipeline.delivers(),
      steps: pipeline.into_steps(),
    }
  }

  /// Steps given whole, valid or not, as automation sends a draft's steps: [`Self::problems`] says
  /// which rules they break. Without a Deliver step until [`Self::set_deliver`] gives one, and delivering changes until
  /// [`Self::set_delivers`] says otherwise.
  pub fn from_steps(description: &str, sources: PipelineSources, steps: Vec<Step>) -> Self {
    Self {
      description: description.to_owned(),
      sources,
      steps,
      deliver: None,
      delivers: Deliverable::default(),
    }
  }

  pub fn description(&self) -> &str {
    &self.description
  }

  pub fn set_description(&mut self, description: &str) {
    description.clone_into(&mut self.description);
  }

  /// The sources the edited pipeline and its steps list; a removed step's leave once it is validated.
  pub fn sources(&self) -> &PipelineSources {
    &self.sources
  }

  pub fn set_sources(&mut self, sources: PipelineSources) {
    self.sources = sources;
  }

  /// The Deliver step the edited pipeline ends in.
  pub const fn deliver(&self) -> Option<&DeliverStep> {
    self.deliver.as_ref()
  }

  /// Ends the edited pipeline in `deliver`, or in no Deliver step.
  pub fn set_deliver(&mut self, deliver: Option<DeliverStep>) {
    self.deliver = deliver;
  }

  /// What the edited pipeline's runs deliver.
  pub const fn delivers(&self) -> Deliverable {
    self.delivers
  }

  pub const fn set_delivers(&mut self, delivers: Deliverable) {
    self.delivers = delivers;
  }

  /// Steps in the order the user arranged them.
  pub fn steps(&self) -> &[Step] {
    &self.steps
  }

  pub fn step(&self, key: &StepKey) -> Option<&Step> {
    self.steps.iter().find(|step| &step.key == key)
  }

  /// Changes step `key` in place; its key stays the same, since other steps and instructions
  /// refer to it. Returns whether the step exists.
  pub fn update(&mut self, key: &StepKey, change: impl FnOnce(&mut Step)) -> bool {
    let Some(step) = self.steps.iter_mut().find(|step| &step.key == key) else {
      return false;
    };
    change(step);
    step.key = key.clone();
    true
  }

  /// Adds an agent step without a profile, after `after` (or at the end) and depending on it,
  /// and returns its key.
  pub fn add_step(&mut self, name: Name, after: Option<&StepKey>) -> StepKey {
    let key = self.unique_key(ADDED_KEY);
    let step = Step {
      key: key.clone(),
      name,
      kind: StepKind::Agent {
        profile: None,
        repository: RepositoryAccess::ReadOnly,
        model: None,
        permissions: StepPermissions::default(),
      },
      instructions: PromptText::new("").expect("empty text has no placeholders"),
      depends_on: after.into_iter().cloned().collect(),
      approval_gate: false,
      on_failure: None,
      budget: None,
    };
    let position = after
      .and_then(|after| self.position(after))
      .map_or(self.steps.len(), |index| index + 1);
    self.steps.insert(position, step);
    key
  }

  /// Copies step `key` right after it. The copy has the same dependencies, so it runs in
  /// parallel with the original, and no step depends on it. Returns the copy's key.
  pub fn duplicate_step(&mut self, key: &StepKey) -> Option<StepKey> {
    let position = self.position(key)?;
    let copy_key = self.unique_key(&format!("{key}-{COPY_SUFFIX}"));
    let copy = Step {
      key: copy_key.clone(),
      ..self.steps[position].clone()
    };
    self.steps.insert(position + 1, copy);
    Some(copy_key)
  }

  /// Removes step `key`. Steps that ran after it now run after its dependencies instead, so
  /// the order around it is kept. Failure routes that returned to it are left as they are and
  /// reported by [`Self::validate`] until they are changed. Returns whether the step existed.
  pub fn remove_step(&mut self, key: &StepKey) -> bool {
    let Some(position) = self.position(key) else {
      return false;
    };
    let removed = self.steps.remove(position);
    for step in &mut self.steps {
      if step.depends_on.remove(key) {
        step.depends_on.extend(removed.depends_on.iter().cloned());
      }
    }
    true
  }

  /// Makes step `key` run after `dependency`, or no longer wait for it. Returns whether the
  /// step exists.
  pub fn set_dependency(&mut self, key: &StepKey, dependency: &StepKey, depends: bool) -> bool {
    self.update(key, |step| {
      if depends {
        step.depends_on.insert(dependency.clone());
      } else {
        step.depends_on.remove(dependency);
      }
    })
  }

  /// The edited steps as a pipeline, or the first rule they break. The pipeline keeps its sources and those of the
  /// steps it still has.
  pub fn validate(&self) -> Result<Pipeline, PipelineError> {
    let sources = self.sources.clone().of_steps(&self.steps);
    let pipeline = Pipeline::new(self.steps.clone())?
      .with_description(self.description.clone())
      .with_deliver(self.deliver.clone())
      .with_delivers(self.delivers);
    Ok(
      pipeline
        .with_sources(sources)
        .expect("only the sources of the pipeline's own steps are kept"),
    )
  }

  /// Everything that keeps the steps from being published: no steps, the first rule they
  /// break (a cycle with the steps around it) and every step without a profile or tool.
  pub fn problems(&self) -> Vec<EditProblem> {
    let mut problems = Vec::new();
    if self.steps.is_empty() {
      problems.push(EditProblem::NoSteps);
    }
    match self.validate() {
      Ok(_) => {}
      Err(PipelineError::Cycle(key)) => {
        problems.push(EditProblem::Cycle(self.cycle().unwrap_or_else(|| vec![key])));
      }
      Err(error) => problems.push(EditProblem::Rule(error)),
    }
    problems.extend(self.unassigned_steps().cloned().map(EditProblem::Unassigned));
    problems
  }

  /// A circle of steps waiting on each other, in the order of [`EditProblem::Cycle`]. The
  /// search walks upstream from the last listed step, so the step that closes the circle is
  /// the earliest one in the flow.
  pub fn cycle(&self) -> Option<Vec<StepKey>> {
    let mut done = BTreeSet::new();
    for step in self.steps.iter().rev() {
      let mut stack = Vec::new();
      if let Some(cycle) = self.find_cycle(&step.key, &mut stack, &mut done) {
        return Some(cycle);
      }
    }
    None
  }

  fn find_cycle<'a>(
    &'a self,
    key: &'a StepKey,
    stack: &mut Vec<&'a StepKey>,
    done: &mut BTreeSet<&'a StepKey>,
  ) -> Option<Vec<StepKey>> {
    if done.contains(key) {
      return None;
    }
    if let Some(start) = stack.iter().position(|entry| *entry == key) {
      return Some(stack[start..].iter().rev().map(|&key| key.clone()).collect());
    }
    let step = self.step(key)?;
    stack.push(key);
    for dependency in &step.depends_on {
      if let Some(cycle) = self.find_cycle(dependency, stack, done) {
        return Some(cycle);
      }
    }
    stack.pop();
    done.insert(key);
    None
  }

  /// Steps whose agent profile or tool is not chosen yet; they block publishing, not saving.
  pub fn unassigned_steps(&self) -> impl Iterator<Item = &StepKey> {
    unassigned(&self.steps)
  }

  fn position(&self, key: &StepKey) -> Option<usize> {
    self.steps.iter().position(|step| &step.key == key)
  }

  /// `base` if no step uses it, otherwise the first free `base-2`, `base-3`, …; an added
  /// step's key when `base` is too long to extend.
  fn unique_key(&self, base: &str) -> StepKey {
    let used: BTreeSet<&str> = self.steps.iter().map(|step| step.key.as_str()).collect();
    let candidates = std::iter::once(base.to_owned()).chain((2_u64..).map(|number| format!("{base}-{number}")));
    for candidate in candidates {
      match StepKey::new(&candidate) {
        Ok(key) if !used.contains(key.as_str()) => return key,
        Ok(_) => {}
        Err(_) => break,
      }
    }
    self.unique_key(ADDED_KEY)
  }
}
