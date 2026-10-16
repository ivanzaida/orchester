//! A status item's card on hover (`Tooltip · Orchester Cloud` `tKtTc` in `ZSB6h`, `Tooltip · Orchester's MCP server`
//! `R2dWD6` in `e2DPHR`): a popover's panel (`ui::popover`) that shows above its anchor, its end at the anchor's end,
//! once the pointer rested on the anchor as long as a tooltip waits, and goes when the pointer leaves. It lets the
//! pointer through. [`card`] lays out what such a card says: a title, a state with its dot, what it means and a hint.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::PaletteColor,
  },
  components::{Column, Placement, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, HitTestBehavior, dimension::Dimension},
};

use super::{popover::popover_panel, tooltip::TOOLTIP_DELAY};
use crate::{
  shell::status_bar::dot,
  theme::{ExtraSpacing, ExtraTypography},
};

/// A card's width (`tKtTc`).
const CARD_WIDTH: f32 = 380.0;

/// What a card says, by its element id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardWords {
  pub id: Arc<str>,
  pub title: String,
  /// The state line's dot.
  pub dot: PaletteColor,
  pub state: String,
  pub body: String,
  pub hint: String,
}

/// The card's panel.
pub fn card(words: &CardWords) -> Element {
  let state = Row::new()
    .id(format!("{}-state", words.id))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(dot(words.dot.clone()))
    .child(Text::new(&words.state).variant(ExtraTypography::Prose.role()));
  popover_panel(&words.id, CARD_WIDTH)
    .spacing(ExtraSpacing::Loose.role())
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .child(Text::new(&words.title).variant(ExtraTypography::FormSection.role()))
        .child(state),
    )
    .child(
      Text::new(&words.body)
        .id(format!("{}-body", words.id))
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(
      Text::new(&words.hint)
        .id(format!("{}-hint", words.id))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

/// `anchor` with `words`' card while the pointer rests on it.
pub fn hover_card(ctx: &mut Ctx, anchor: impl Into<Element>, words: CardWords) -> Element {
  let key = format!("hover-card-{}", words.id);
  ctx.mount_keyed_with::<HoverCard>(&key, words, vec![anchor.into()])
}

impl DevtoolsInspectable for CardWords {}

struct HoverCard {
  shown: Signal<bool>,
  timer: Timeout,
  anchor: ElementRef,
}

impl Component for HoverCard {
  type Props = CardWords;

  fn create(ctx: &mut Ctx) -> Self {
    let shown = ctx.signal(false);
    let show = shown.clone();
    Self {
      shown,
      timer: ctx.create_timeout(TOOLTIP_DELAY, move || show.set(true)),
      anchor: ctx.element_ref(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let words = ctx.props::<CardWords>().clone();
    let (rest, leave, hide) = (self.timer.clone(), self.timer.clone(), self.shown.clone());
    let anchor = Row::new()
      .ref_element(self.anchor.clone())
      .align_items(Alignment::Center)
      .on_mouse_enter(move || rest.restart())
      .on_mouse_leave(move || {
        leave.cancel();
        hide.set(false);
      })
      .with_children(ctx.children().iter().cloned());
    if !self.shown.get() {
      return anchor;
    }
    anchor.child(
      Popup::new(self.anchor.clone(), card(&words))
        .open_when(true)
        .placement(Placement::TopEnd)
        // Over its anchor, lurq counts the offset away from it.
        .offset(0.0, ExtraSpacing::Snug.value())
        .hit_test(HitTestBehavior::None)
        .dismiss_on_outside_click(false)
        .dismiss_on_escape(false),
    )
  }
}
