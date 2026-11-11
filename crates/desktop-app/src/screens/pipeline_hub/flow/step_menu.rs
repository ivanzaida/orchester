//! A card's menu (P4 `naN3G`) opened from outside the flow, for one step: the open step's ⋯ in its head
//! ([`crate::screens::pipeline_hub::step`]). It is the card's menu itself ([`super::menu`]), drawn from a state of its
//! own: whether it is open and why the last move it made was refused.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Popup,
  core::{ElementRef, Signal},
};
use orchester_core::resource::{PipelineEdit, StepKey};
use orchester_workspace::pipelines::flow::{PipelineFlow, StepCard};

use super::{
  menu::{Menu, OpensMade, popup},
  reorder::{Cards, Drive},
  state::Reorder,
};
use crate::screens::pipeline_hub::edit::Edit;

/// The state of one step's menu outside the flow.
#[derive(Clone)]
pub(in crate::screens::pipeline_hub) struct StepMenu {
  reorder: Reorder,
}

/// What the menu acts on: the panel's edit, what it shows, its flow and the step's card there.
pub(in crate::screens::pipeline_hub) struct Target<'a> {
  pub edit: &'a Edit,
  pub current: &'a Arc<PipelineEdit>,
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

  /// Opens or closes the menu; what the last move was refused for goes.
  pub(in crate::screens::pipeline_hub) fn toggle(&self) {
    self.reorder.refused.set(None);
    self.reorder.menu_open.update(|open| *open = !*open);
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
      current: target.current.clone(),
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

  /// Why the last move the menu made on step `key` of `flow` was refused, in the card's words; reading it subscribes
  /// the caller.
  pub(in crate::screens::pipeline_hub) fn refusal(
    &self,
    ctx: &Ctx,
    (flow, key): (&PipelineFlow, &StepKey),
  ) -> Option<Arc<str>> {
    let refused = self.reorder.refused.get()?;
    let cards = Cards::of(flow);
    Some(ctx.t_args(
      "pipeline_hub.flow.move.refused",
      [
        ("name", cards.name(key).to_string()),
        ("reason", cards.reason(ctx, &refused.error, Some(key)).to_string()),
      ],
    ))
  }
}
