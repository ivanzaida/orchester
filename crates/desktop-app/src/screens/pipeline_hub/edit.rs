//! The edit the panel of a pipeline holds (`Pipelines · v2` P1): every change to the pipeline and its steps is made on
//! one in-memory [`PipelineEdit`] and nothing is stored until *Save* ([`super::save`]); *Cancel* drops it. The flow and
//! the settings draw the edit, not the stored pipeline, while one is open.
//!
//! [`Edit`] is what the parts of the panel are given. A part that changes something calls [`Edit::change`] with what to
//! do to the [`PipelineEdit`], whatever it is: the settings rows do today, a card's move (PV2-06), a step's fields
//! (PV2-07, PV2-08) and a restored version (PV2-11) do the same, and *Save*, *Cancel*, the leave guard and the list's
//! *Unsaved changes* follow from the one edit with nothing more to wire.

use std::{collections::BTreeSet, sync::Arc};

use lurq::{
  app::component::DevtoolsInspectable,
  core::{Ref, Signal},
};
use orchester_core::resource::{Pipeline, PipelineEdit, Step, StepKey};
use orchester_workspace::pipelines::local_edit::Held;

/// What the panel holds of the pipeline.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Holding {
  /// The edit; `None` while the panel shows what is stored.
  pub held: Option<Held>,
  /// What was stored when a save ended: the panel shows what it saved until the hub lists another pipeline.
  pub saved_over: Option<Arc<Pipeline>>,
  /// The steps whose *Waits for* row the person opened (*Wait for some steps only…*) or changed since the edit began:
  /// the row stays for them though they wait for the whole stage before them (P16b `P1853`).
  pub waits_shown: BTreeSet<StepKey>,
}
impl DevtoolsInspectable for Holding {}

/// The panel's way to the edit it holds, over the pipeline as stored now.
#[derive(Clone)]
pub(super) struct Edit {
  holding: Signal<Holding>,
  stored: Arc<Pipeline>,
  /// The pipeline as stored now, which the panel keeps current. A handle kept by a field that is not drawn again when
  /// the pipeline is stored anew (a select, a box) still starts an edit from what is stored, not from what was.
  latest: Option<Ref<Option<Arc<Pipeline>>>>,
}

impl Edit {
  pub(super) fn new(holding: &Signal<Holding>, stored: Arc<Pipeline>) -> Self {
    Self {
      holding: holding.clone(),
      stored,
      latest: None,
    }
  }

  /// This edit starting from what `latest` holds when a change starts it, rather than from what was stored when the
  /// edit was made.
  pub(super) fn following(mut self, latest: &Ref<Option<Arc<Pipeline>>>) -> Self {
    self.latest = Some(latest.clone());
    self
  }

  /// What a change starts from: the pipeline as stored now.
  fn base(&self) -> Arc<Pipeline> {
    self
      .latest
      .as_ref()
      .and_then(Ref::get)
      .unwrap_or_else(|| self.stored.clone())
  }

  /// What the panel shows: the edit, or the stored pipeline while there is none. Reading it in a render subscribes the
  /// panel to the edit.
  pub(super) fn current(&self) -> PipelineEdit {
    self.holding.get().held.map_or_else(
      || PipelineEdit::new(Pipeline::clone(&self.stored)),
      |held| held.edit().clone(),
    )
  }

  /// [`Self::current`] for a handler: reading it subscribes nothing.
  pub(super) fn current_now(&self) -> PipelineEdit {
    self.holding.get_untracked().held.map_or_else(
      || PipelineEdit::new(Pipeline::clone(&self.stored)),
      |held| held.edit().clone(),
    )
  }

  /// Makes `change` to the edit, which it starts from the stored pipeline (the stored draft, else the latest version)
  /// when there is none yet.
  pub(super) fn change(&self, change: impl FnOnce(&mut PipelineEdit)) {
    let stored = self.base();
    self.holding.update(|holding| {
      holding.held.get_or_insert_with(|| Held::start(stored)).change(change);
      holding.saved_over = None;
    });
  }

  /// *Restore*: puts `version` into the edit, replacing its changes; the edit starts from the stored pipeline when
  /// there is none yet.
  pub(super) fn restore(&self, version: &Pipeline) {
    let stored = self.base();
    self.holding.update(|holding| {
      holding.held.get_or_insert_with(|| Held::start(stored)).restore(version);
      holding.saved_over = None;
    });
  }

  /// Whether the edit differs from what it started from. Reading it in a render subscribes the panel to the edit.
  pub(super) fn is_changed(&self) -> bool {
    self.holding.get().held.is_some_and(|held| held.is_changed())
  }

  /// Step `key` as the edit began from it (what was stored then, or what the last save stored); `None` while the panel
  /// holds no edit or the pipeline then had no such step. What the edit changed since, such as a model override a
  /// choice dropped, is read against it until *Save* or *Cancel*. Reading it in a render subscribes the panel to the
  /// edit.
  pub(super) fn started_step(&self, key: &StepKey) -> Option<Step> {
    self
      .holding
      .get()
      .held
      .and_then(|held| held.base().steps().iter().find(|step| &step.key == key).cloned())
  }

  /// Whether the panel holds an edit, changed or not: after a save, the edit is what was saved.
  pub(super) fn holds(&self) -> bool {
    self.holding.get().held.is_some()
  }

  /// What *Save* stores, for a handler: the edit, or, while there is none, the stored pipeline as it stands (a stored
  /// draft nobody has changed is saved as it is, which publishes it).
  pub(super) fn to_save(&self) -> Held {
    self
      .holding
      .get_untracked()
      .held
      .unwrap_or_else(|| Held::start(self.base()))
  }

  /// The description the panel shows, for a handler.
  pub(super) fn description_now(&self) -> String {
    self.holding.get_untracked().held.map_or_else(
      || self.stored.description().to_owned(),
      |held| held.edit().description().to_owned(),
    )
  }

  /// Shows step `key`'s *Waits for* row until the edit is saved or dropped.
  pub(super) fn show_waits(&self, key: &StepKey) {
    if self.holding.get_untracked().waits_shown.contains(key) {
      return;
    }
    self.holding.update(|holding| {
      holding.waits_shown.insert(key.clone());
    });
  }

  /// Whether the person opened or changed step `key`'s *Waits for* row. Reading it in a render subscribes the panel to
  /// the edit.
  pub(super) fn shows_waits(&self, key: &StepKey) -> bool {
    self.holding.get().waits_shown.contains(key)
  }

  /// *Cancel*: drops the edit, so the panel shows what is stored again.
  pub(super) fn cancel(&self) {
    self.holding.set(Holding::default());
  }
}

/// The change a test or the QA app armed for pipeline `id` ([`crate::test_support::pipeline_edit`]), made to the edit
/// `holding` holds of `stored` as if the person had made it: steps the hub never makes reach the panel this way. Only
/// the `test-support` feature builds it.
#[cfg(feature = "test-support")]
pub(super) fn armed(holding: &Signal<Holding>, id: orchester_core::id::ResourceId, stored: Option<Arc<Pipeline>>) {
  if let (Some(change), Some(stored)) = (crate::test_support::pipeline_edit::take(id), stored) {
    Edit::new(holding, stored).change(change);
  }
}
