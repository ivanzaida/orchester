//! What the pipeline editor holds between renders: the steps being edited, which of their
//! revisions is stored as the draft, and the versions around it. Edits are saved as the draft
//! automatically while they can be: a step without a profile can be saved, a cycle cannot, so
//! such an edit stays on screen, unsaved, until it is fixed.

use std::{collections::BTreeSet, sync::Arc};

use orchester_core::{
  id::ResourceId,
  resource::{Draft, EditProblem, Pipeline, PipelineEdit, StepKey, VersionNumber},
  time::Timestamp,
};

use super::data::{Assignables, OpenedPipeline, Published};

/// How the edits relate to what is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveState {
  /// Every edit is stored as the draft, or there is no draft and nothing was edited.
  Saved,
  /// An edit is not stored yet.
  Pending,
  /// The steps break a rule a draft must keep, or a field holds text that cannot be used; the
  /// last saved edit is what is stored.
  Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorState {
  opened: Arc<OpenedPipeline>,
  edit: PipelineEdit,
  draft_saved_at: Option<Timestamp>,
  based_on: Option<VersionNumber>,
  latest: Option<Published>,
  revision: u64,
  saved_revision: u64,
  /// Inspector fields whose text cannot be used, by step: the step keeps its last valid value.
  invalid_fields: BTreeSet<(StepKey, &'static str)>,
  /// Set once the draft was published in this editor, for the "published just now" notice.
  just_published: bool,
}

impl lurq::app::component::DevtoolsInspectable for EditorState {}

impl EditorState {
  pub fn new(opened: Arc<OpenedPipeline>) -> Self {
    Self {
      edit: PipelineEdit::new(opened.body.clone()),
      draft_saved_at: opened.draft_saved_at,
      based_on: opened.based_on,
      latest: opened.latest.clone(),
      opened,
      revision: 0,
      saved_revision: 0,
      invalid_fields: BTreeSet::new(),
      just_published: false,
    }
  }

  pub fn id(&self) -> ResourceId {
    self.opened.id
  }

  pub fn name(&self) -> &str {
    &self.opened.name
  }

  pub fn slug(&self) -> &str {
    &self.opened.slug
  }

  pub fn used_by(&self) -> usize {
    self.opened.used_by
  }

  pub fn assignables(&self) -> &Assignables {
    &self.opened.assignables
  }

  pub const fn edit(&self) -> &PipelineEdit {
    &self.edit
  }

  pub const fn has_draft(&self) -> bool {
    self.draft_saved_at.is_some()
  }

  /// Whether there are changes since the latest version: a stored draft, or edits not stored
  /// yet. The editor then shows the draft's number instead of the published version.
  pub const fn has_changes(&self) -> bool {
    self.has_draft() || self.revision > self.saved_revision
  }

  pub const fn draft_saved_at(&self) -> Option<Timestamp> {
    self.draft_saved_at
  }

  pub const fn latest(&self) -> Option<&Published> {
    self.latest.as_ref()
  }

  pub const fn just_published(&self) -> bool {
    self.just_published
  }

  /// Number the draft gets when published; the next edit starts it when there is no draft.
  pub fn draft_number(&self) -> u32 {
    self.based_on.map_or(1, |version| version.get() + 1)
  }

  /// Applies an edit to the steps. An edit that changes nothing is not a new revision.
  pub fn apply<R>(&mut self, change: impl FnOnce(&mut PipelineEdit) -> R) -> R {
    let before = self.edit.clone();
    let result = change(&mut self.edit);
    if self.edit != before {
      self.revision += 1;
      self.just_published = false;
    }
    result
  }

  pub fn problems(&self) -> Vec<EditProblem> {
    self.edit.problems()
  }

  pub fn set_field_invalid(&mut self, step: &StepKey, field: &'static str, invalid: bool) {
    let entry = (step.clone(), field);
    if invalid {
      self.invalid_fields.insert(entry);
    } else {
      self.invalid_fields.remove(&entry);
    }
  }

  /// Forgets unusable field text of `step`, when the inspector shows another step and the
  /// text is gone.
  pub fn clear_invalid_fields(&mut self, step: &StepKey) {
    self.invalid_fields.retain(|(key, _)| key != step);
  }

  pub fn save_state(&self) -> SaveState {
    if !self.invalid_fields.is_empty() || self.edit.validate().is_err() {
      SaveState::Invalid
    } else if self.revision > self.saved_revision {
      SaveState::Pending
    } else {
      SaveState::Saved
    }
  }

  /// The draft to store for the current revision, if it can be stored and is not yet.
  pub fn draft_to_save(&self, now: Timestamp) -> Option<(u64, Draft<Pipeline>)> {
    if self.save_state() != SaveState::Pending {
      return None;
    }
    let pipeline = self.edit.validate().ok()?;
    Some((self.revision, Draft::new(pipeline, self.based_on, now)))
  }

  /// `revision` was stored as the draft at `at`.
  pub fn saved(&mut self, revision: u64, at: Timestamp) {
    self.saved_revision = self.saved_revision.max(revision);
    self.draft_saved_at = Some(at);
  }

  /// Whether the stored draft can be published now: every edit is saved and nothing blocks
  /// publishing.
  pub fn can_publish(&self) -> bool {
    self.has_draft() && self.save_state() == SaveState::Saved && self.problems().is_empty()
  }

  /// The stored draft `body` was published as `number` at `at`; the next edit starts a new
  /// draft from it. Edits made while publishing stay pending and go into that new draft.
  pub fn published(&mut self, number: VersionNumber, at: Timestamp, body: Pipeline) {
    self.draft_saved_at = None;
    self.based_on = Some(number);
    self.latest = Some(Published { number, at, body });
    self.just_published = true;
  }
}
