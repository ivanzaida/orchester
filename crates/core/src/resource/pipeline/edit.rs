//! Editing the steps of a pipeline draft. Edits may break pipeline rules on the way — a step
//! returning to a removed one, a cycle, no steps at all — so the edited steps are kept as they
//! are and [`PipelineEdit::validate`] says which rule is broken. Only a valid list becomes a
//! [`Pipeline`] that can be saved.

use std::collections::BTreeSet;

use super::{Pipeline, PipelineError, RepositoryAccess, Step, StepKey, StepKind, unassigned};
use crate::{name::Name, resource::prompt::PromptText};

/// Base of the keys given to added steps: `step`, `step-2`, …
const ADDED_KEY: &str = "step";
/// Suffix of the key of a duplicated step: `review-copy`, `review-copy-2`, …
const COPY_SUFFIX: &str = "copy";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineEdit {
  steps: Vec<Step>,
}

impl PipelineEdit {
  pub fn new(pipeline: Pipeline) -> Self {
    Self {
      steps: pipeline.into_steps(),
    }
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

  /// The edited steps as a pipeline, or the first rule they break.
  pub fn validate(&self) -> Result<Pipeline, PipelineError> {
    Pipeline::new(self.steps.clone())
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
