//! What the panel does around a save: it keeps what is stored current, tells the list and the bar of the edit it
//! shows, pins what the last save said over the bar, and makes *Save*, *Cancel* and *Discard changes* do what they
//! say.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::resource::{Pipeline, PipelineEdit};
use orchester_workspace::pipelines::{flow::FlowRead, ready::step_problems};

use super::{PipelinePanel, Restored};
use crate::{
  screens::{
    local_hub::save_bar::Save as SaveState,
    pipeline_hub::{
      bar::{self, Said},
      edit::{Edit, Holding},
      leave::{Edited, Unsaved, mark_unsaved},
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

  /// What the panel follows of the edit it shows: whether it has changes, which a save's note and the description's
  /// box then follow. The question before *Restore* goes with the version it was asked about, and what *Restore* put
  /// in is forgotten once the edit holds no change.
  pub(super) fn follow_edit(&self, current: &PipelineEdit, changed: bool) {
    if self.restoring.get_untracked() && self.viewing.get_untracked().is_none() {
      self.restoring.set(false);
    }
    if !changed && self.restored.get_untracked().0.is_some() {
      self.restored.set(Restored(None));
    }
    if changed {
      self.notes.forget_stale(current);
    } else if self.description.get_untracked() != current.description() {
      self.description.set(current.description().to_owned());
    }
  }

  /// Tells the list of the edit the panel holds while it is `unsaved` (text the check refuses is unsaved too, though
  /// the edit does not hold it: leaving would drop it): how many of its steps cannot run (`read`), which its row says,
  /// and whether *Save* is off for it (`save_off`), which the question about leaving says. Gives how many steps cannot
  /// run.
  pub(super) fn tell_list(&self, unsaved: bool, read: &FlowRead, save_off: bool) -> usize {
    let steps_not_ready = step_problems(read).len();
    let edited = unsaved.then_some(Edited {
      pipeline: self.id,
      steps_not_ready,
      save_off,
    });
    mark_unsaved(&self.unsaved, edited);
    steps_not_ready
  }

  /// What is pinned over the bar: the question a save asks, and what the last save said. `runs`: what was saved can
  /// run, which the saved version's notice says. While a version is shown read-only (`viewing`) the bar is not there,
  /// so neither is what the last save said, which speaks of it; it shows again on *Back*.
  pub(super) fn pinned(
    &self,
    ctx: &mut Ctx,
    entry: &Entry,
    asked: Option<&Asked>,
    (runs, viewing): (bool, bool),
  ) -> Vec<Element> {
    let mut pinned: Vec<Element> = Vec::new();
    if let Some(asked) = asked {
      let restored = self.restored.get_untracked().0;
      pinned.push(save::question(ctx, asked, (&self.save, &self.pending), restored));
    }
    if viewing {
      return pinned;
    }
    let told = self.notes.told();
    match (told.refused, told.made) {
      (Some((reason, _)), _) => pinned.push(bar::notice(ctx, entry, &Said::Refused(&reason))),
      (None, Some(version)) => pinned.push(bar::notice(ctx, entry, &Said::Made(version, runs))),
      (None, None) => {}
    }
    pinned
  }

  /// What *Save* does now: nothing while a save waits, off with `off`'s reason (text the check refuses, which the edit
  /// does not hold, so saving would store what the box no longer says), nothing while nothing differs from the latest
  /// version, else it publishes what the panel holds.
  pub(super) fn save_now(&self, edit: &Edit, changed: bool, (waiting, off): (bool, Option<String>)) -> SaveState {
    if self.save.state().get().is_pending() {
      return SaveState::Saving;
    }
    if waiting {
      return SaveState::Unchanged;
    }
    if let Some(why) = off {
      return SaveState::Waits(why.into());
    }
    if !changed {
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
      if unsaved.pipeline() != Some(id) && holds {
        holding.set(Holding::default());
        put_back();
      }
    });
  }
}
