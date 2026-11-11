//! A card's menu (P4 `naN3G`) opened from outside the flow, for one step: the open step's ⋯ in its head
//! ([`crate::screens::pipeline_hub::step`]). It is the card's menu itself ([`super::menu`]), drawn from a state of its
//! own: whether it is open. Alt ↑ and Alt ↓ move the step as they move the card the keys are on.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, events::KeyboardEvent},
  components::Popup,
  core::{ElementRef, Signal},
};
use orchester_core::resource::StepKey;
use orchester_workspace::pipelines::flow::{PipelineFlow, StepCard};

use super::{
  menu::{Menu, OpensMade, popup},
  reorder::{Cards, Drive, keyed_move},
  state::Reorder,
};
use crate::screens::pipeline_hub::edit::Edit;

/// The state of one step's menu outside the flow.
#[derive(Clone)]
pub(in crate::screens::pipeline_hub) struct StepMenu {
  reorder: Reorder,
}

/// What the menu acts on: the panel's edit, its flow and the step's card there.
pub(in crate::screens::pipeline_hub) struct Target<'a> {
  pub edit: &'a Edit,
  pub flow: &'a PipelineFlow,
  pub card: &'a StepCard,
}

impl StepMenu {
  pub(in crate::screens::pipeline_hub) fn new(ctx: &mut Ctx) -> Self {
    Self {
      reorder: Reorder::new(ctx),
    }
  }

  /// Whether the menu is open, and its signal, which the menu's rows close.
  pub(in crate::screens::pipeline_hub) fn open(&self) -> &Signal<bool> {
    &self.reorder.menu_open
  }

  /// Opens or closes the menu.
  pub(in crate::screens::pipeline_hub) fn toggle(&self) {
    self.reorder.menu_open.update(|open| *open = !*open);
  }

  /// Alt ↑ and Alt ↓ move step `key` of `edit` as they move the card the keys are on in the flow.
  pub(in crate::screens::pipeline_hub) fn keys(
    &self,
    edit: &Edit,
    key: &StepKey,
  ) -> impl Fn(KeyboardEvent) + Send + Sync + use<> {
    let (reorder, edit, key) = (self.reorder.clone(), edit.clone(), key.clone());
    move |event: KeyboardEvent| {
      let Some(change) = keyed_move(&event) else {
        return;
      };
      event.prevent_default();
      let drive = Drive {
        reorder: reorder.clone(),
        current: Arc::new(edit.current_now()),
        edit: edit.clone(),
      };
      drive.apply(&key, &change, false);
    }
  }

  /// A save is under way or a question about it waits: the menu changes nothing and closes.
  pub(in crate::screens::pipeline_hub) fn lock(&self, locked: bool) {
    *self.reorder.locked.lock() = locked;
    if locked && self.reorder.menu_open.get_untracked() {
      self.reorder.menu_open.set(false);
    }
  }

  /// The card's menu for `target`, at `anchor`; a step it makes goes to `opens`.
  pub(in crate::screens::pipeline_hub) fn popup(
    &self,
    ctx: &mut Ctx,
    target: &Target<'_>,
    (anchor, opens): (ElementRef, OpensMade),
  ) -> Popup {
    let drive = Drive {
      reorder: self.reorder.clone(),
      edit: target.edit.clone(),
      current: Arc::new(target.edit.current()),
    };
    let cards = Cards::of(target.flow);
    let menu = Menu {
      drive: &drive,
      flow: target.flow,
      cards: &cards,
      opens: Some(opens),
    };
    popup(ctx, &menu, target.card, anchor)
  }
}
