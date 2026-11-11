//! What the panel does around a save: it keeps what is stored current, tells the list and the bar of the edit it
//! shows, pins what the last save said over the bar, and makes *Save*, *Cancel* and *Discard changes* do what they
//! say.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::resource::{Pipeline, PipelineEdit};

use super::{PipelinePanel, Restored};
use crate::{
  screens::{
    local_hub::save_bar::Save as SaveState,
    pipeline_hub::{
      bar::{self, Said},
      edit::{Edit, Holding},
      leave::{Unsaved, mark_unsaved},
      model::Entry,
      save::{self, Ask, Asked},
    },
  },
  ui::pressable::Press,
};

impl PipelinePanel {
  /// Keeps `stored` current. After a save the panel shows what it saved until the hub lists another pipeline; then what
  /// the hub lists.
  pub(super) fn settle(&self, stored: &Arc<Pipeline>) {
    self.stored.set(Some(stored.clone()));
    let holding = self.holding.get_untracked();
    if holding.saved_over.as_ref().is_some_and(|over| over != stored) {
      self.holding.set(Holding::default());
    }
  }

  /// What the panel tells the list and the bar of the edit it shows: whether it has changes, which a save's note and
  /// the description's box then follow. Text the check refuses is unsaved too (`unsaved`), though the edit does not
  /// hold it: leaving would drop it. The question before *Restore* goes with the version it was asked about, and what
  /// *Restore* put in is forgotten once the edit holds no change.
  pub(super) fn follow_edit(&self, current: &PipelineEdit, (changed, unsaved): (bool, bool)) {
    if self.restoring.get_untracked() && self.viewing.get_untracked().is_none() {
      self.restoring.set(false);
    }
    if !changed && self.restored.get_untracked().0.is_some() {
      self.restored.set(Restored(None));
    }
    mark_unsaved(&self.unsaved, self.id, unsaved);
    if changed {
      self.notes.forget_stale(current);
    } else if self.description.get_untracked() != current.description() {
      self.description.set(current.description().to_owned());
    }
  }

  /// What is pinned over the bar: the question a save asks, and what the last save said. `runs`: what was saved can
  /// run, which the saved version's notice says.
  pub(super) fn pinned(&self, ctx: &mut Ctx, entry: &Entry, asked: Option<&Asked>, runs: bool) -> Vec<Element> {
    let mut pinned: Vec<Element> = Vec::new();
    if let Some(asked) = asked {
      let restored = self.restored.get_untracked().0;
      pinned.push(save::question(ctx, asked, (&self.save, &self.pending), restored));
    }
    let told = self.notes.told();
    match (told.refused, told.kept, told.made) {
      (Some((reason, _)), ..) => pinned.push(bar::notice(ctx, entry, &Said::Refused(&reason))),
      (None, Some(why), _) => pinned.push(bar::notice(ctx, entry, &Said::Kept(&why))),
      (None, None, Some(version)) => pinned.push(bar::notice(ctx, entry, &Said::Made(version, runs))),
      (None, None, None) => {}
    }
    pinned
  }

  /// What *Save* does now: nothing while a save waits, off with `off`'s reason (text the check refuses, which the edit
  /// does not hold, so saving would store what the box no longer says; or a draft publishing would only keep again),
  /// nothing while nothing differs and no draft waits to be published, else it stores and publishes what the panel
  /// holds.
  pub(super) fn save_now(
    &self,
    edit: &Edit,
    (changed, unpublished): (bool, bool),
    (waiting, off): (bool, Option<String>),
  ) -> SaveState {
    if self.save.state().get().is_pending() {
      return SaveState::Saving;
    }
    if waiting {
      return SaveState::Unchanged;
    }
    if let Some(why) = off {
      return SaveState::Waits(why.into());
    }
    if !changed && !unpublished {
      return SaveState::Unchanged;
    }
    let (edit, save, notes) = (edit.clone(), self.save.clone(), self.notes.clone());
    SaveState::Ready(Press::run(move || {
      let held = edit.to_save();
      notes.begin(held.edit());
      save.run(Ask {
        held,
        allowed: Vec::new(),
      });
    }))
  }

  /// *Cancel*: the panel shows what is stored again at once.
  pub(super) fn cancel_now(&self, edit: &Edit) -> Press {
    let (edit, put_back) = (edit.clone(), self.put_back());
    Press::run(move || {
      edit.cancel();
      put_back();
    })
  }

  /// What dropping the edit puts back besides the edit: the description's box, what the last save said, text the
  /// check refused and what *Restore* put in.
  fn put_back(&self) -> impl Fn() + Send + Sync + use<> {
    let (description, stored, pending, notes, (reset, faulty, restored)) = (
      self.description.clone(),
      self.stored.clone(),
      self.pending.clone(),
      self.notes.clone(),
      (self.reset.clone(), self.faulty.clone(), self.restored.clone()),
    );
    move || {
      if let Some(stored) = stored.get() {
        description.set(stored.description().to_owned());
      }
      pending.set(None);
      notes.forget();
      reset.update(|resets| *resets += 1);
      faulty.set(Vec::new());
      restored.set(Restored(None));
    }
  }

  /// What is typed in the description's box is the edit's description, whether the text came from a person or was set;
  /// a box that already says what the edit says changes nothing.
  pub(super) fn follow_typed(&self, ctx: &mut Ctx) {
    let (holding, stored) = (self.holding.clone(), self.stored.clone());
    ctx.watch(&self.description, move |typed: &String| {
      let Some(stored) = stored.get() else {
        return;
      };
      let edit = Edit::new(&holding, stored);
      if edit.description_now() != *typed {
        edit.change(|current| current.set_description(typed));
      }
    });
  }

  /// The hub's question was answered *Discard changes*: the edit goes with them, and text the check refused, whether
  /// the panel stays (another tab that holds this pipeline) or goes.
  pub(super) fn follow_discard(&self, ctx: &mut Ctx) {
    let (holding, faulty, put_back, id) = (self.holding.clone(), self.faulty.clone(), self.put_back(), self.id);
    ctx.watch(&self.unsaved, move |unsaved: &Unsaved| {
      let holds =
        holding.get_untracked().held.is_some_and(|held| held.is_changed()) || !faulty.get_untracked().is_empty();
      if unsaved.0 != Some(id) && holds {
        holding.set(Holding::default());
        put_back();
      }
    });
  }
}
