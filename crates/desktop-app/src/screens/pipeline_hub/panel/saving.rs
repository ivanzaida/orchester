//! What the panel does around a save: it keeps what is stored current, tells the list and the bar of the edit it
//! shows, pins what the last save said over the bar, and makes *Save*, *Cancel* and *Discard changes* do what they
//! say.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::resource::{Pipeline, PipelineEdit};

use super::PipelinePanel;
use crate::{
  screens::{
    local_hub::save_bar::Save as SaveState,
    pipeline_hub::{
      bar::{self, Said},
      edit::{Edit, Holding},
      leave::{Unsaved, mark_unsaved},
      model::Entry,
      save::{self, Ask, Asked},
      step,
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
  /// the description's box then follow. The question before *Restore* goes with the version it was asked about.
  pub(super) fn follow_edit(&self, current: &PipelineEdit, changed: bool) {
    if self.restoring.get_untracked() && self.viewing.get_untracked().is_none() {
      self.restoring.set(false);
    }
    mark_unsaved(&self.unsaved, self.id, changed);
    if changed {
      self.notes.forget_stale(current);
    } else if self.description.get_untracked() != current.description() {
      self.description.set(current.description().to_owned());
    }
  }

  /// What is pinned over the bar: the question a save asks, and what the last save said.
  pub(super) fn pinned(&self, ctx: &mut Ctx, entry: &Entry, asked: Option<&Asked>) -> Vec<Element> {
    let mut pinned: Vec<Element> = Vec::new();
    if let Some(asked) = asked {
      pinned.push(save::question(ctx, asked, (&self.save, &self.pending)));
    }
    let told = self.notes.told();
    match (told.refused, told.kept, told.made) {
      (Some((reason, _)), ..) => pinned.push(bar::notice(ctx, entry, &Said::Refused(&reason))),
      (None, Some(why), _) => pinned.push(bar::notice(ctx, entry, &Said::Kept(&why))),
      (None, None, Some(version)) => pinned.push(bar::notice(ctx, entry, &Said::Made(version))),
      (None, None, None) => {}
    }
    pinned
  }

  /// What *Save* does now: nothing while nothing differs and no draft waits to be published, else it stores and
  /// publishes what the panel holds.
  pub(super) fn save_now(&self, edit: &Edit, (changed, unpublished): (bool, bool), waiting: bool) -> SaveState {
    if self.save.state().get().is_pending() {
      return SaveState::Saving;
    }
    if !changed && !unpublished {
      return SaveState::Unchanged;
    }
    // Text that is not valid is not in the edit, so saving now would store what the box no longer says.
    if waiting || step::holds_invalid(&self.faulty, &edit.current()) {
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

  /// What dropping the edit puts back besides the edit: the description's box, and what the last save said.
  fn put_back(&self) -> impl Fn() + Send + Sync + use<> {
    let (description, stored, pending, notes, (reset, faulty)) = (
      self.description.clone(),
      self.stored.clone(),
      self.pending.clone(),
      self.notes.clone(),
      (self.reset.clone(), self.faulty.clone()),
    );
    move || {
      if let Some(stored) = stored.get() {
        description.set(stored.description().to_owned());
      }
      pending.set(None);
      notes.forget();
      reset.update(|resets| *resets += 1);
      faulty.set(Vec::new());
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

  /// The hub's question was answered *Discard changes*: the edit goes with them, whether the panel stays (another tab
  /// that holds this pipeline) or goes.
  pub(super) fn follow_discard(&self, ctx: &mut Ctx) {
    let (holding, put_back, id) = (self.holding.clone(), self.put_back(), self.id);
    ctx.watch(&self.unsaved, move |unsaved: &Unsaved| {
      let holds = holding.get_untracked().held.is_some_and(|held| held.is_changed());
      if unsaved.0 != Some(id) && holds {
        holding.set(Holding::default());
        put_back();
      }
    });
  }
}
