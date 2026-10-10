//! Option card (`Option card/off` `Imh8O`, `Option card/on` `g8tsS`): one choice of a few, as a card with its
//! radio dot, a 14 icon and its title in 12.5/500, and under them what it means in 12/1.45
//! `text_secondary`; 12 over and under, 14 at its sides, radius 6. Off it is `surface_input` in a `border`
//! stroke with an empty dot; chosen, `surface_raised` in an `accent` stroke with the dot filled. A choice that
//! cannot be made is faded, shows a lock in place of its dot and says why under its description (`Reason`
//! `a7DoR`). It is a form control: Tab reaches it and Enter or Space chooses it.

use std::sync::Arc;

use lurq::{
  app::{
    events::mouse_event::MouseEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Button, Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::{Focus, inert},
};
use crate::theme::{DISABLED_OPACITY, ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize};

/// The radio dot (`Dot` `ABOwN`) and its chosen center (`Inner` `E4gxfi`).
const DOT_SIZE: f32 = 16.0;
const DOT_CENTER: f32 = 6.0;

/// Whether the card can be chosen.
enum Choice {
  Choose(Arc<dyn Fn() + Send + Sync>),
  Unavailable(Arc<str>),
}

pub struct OptionCard {
  id: Arc<str>,
  icon: Icon,
  title: Arc<str>,
  description: Arc<str>,
  chosen: bool,
  choice: Choice,
}

impl OptionCard {
  /// A card that `choose` picks; `chosen` when it is the current choice.
  pub fn new(
    id: impl Into<Arc<str>>,
    (icon, title): (Icon, impl Into<Arc<str>>),
    description: impl Into<Arc<str>>,
    chosen: bool,
    choose: impl Fn() + Send + Sync + 'static,
  ) -> Self {
    Self {
      id: id.into(),
      icon,
      title: title.into(),
      description: description.into(),
      chosen,
      choice: Choice::Choose(Arc::new(choose)),
    }
  }

  /// The choice cannot be made, for `reason`.
  #[must_use]
  pub fn unavailable(mut self, reason: impl Into<Arc<str>>) -> Self {
    self.choice = Choice::Unavailable(reason.into());
    self
  }

  fn dot(&self) -> Element {
    if matches!(self.choice, Choice::Unavailable(_)) {
      return icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted).into();
    }
    let dot = Row::new()
      .width(Dimension::Px(DOT_SIZE))
      .height(Dimension::Px(DOT_SIZE))
      .rounded(ExtraRadius::Full.role())
      .align_items(Alignment::Center)
      .justify(Justify::Center);
    if self.chosen {
      return dot
        .background(PaletteColor::Accent)
        .child(
          Row::new()
            .width(Dimension::Px(DOT_CENTER))
            .height(Dimension::Px(DOT_CENTER))
            .rounded(ExtraRadius::Full.role())
            .background(ExtraColor::OnAccent.role()),
        )
        .into();
    }
    dot
      .background(PaletteColor::SurfaceInput)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .into()
  }
}

impl From<OptionCard> for Element {
  fn from(card: OptionCard) -> Self {
    let glyph = if card.chosen {
      PaletteColor::TextPrimary
    } else {
      PaletteColor::TextSecondary
    };
    let reason = match &card.choice {
      Choice::Unavailable(reason) => Some(
        Text::new(reason)
          .id(format!("{}-reason", card.id))
          .variant(ExtraTypography::Prose.role())
          .color(PaletteColor::TextMuted),
      ),
      Choice::Choose(_) => None,
    };
    let text = Column::new()
      .flex(1.0)
      .spacing(SpacingSize::Xs)
      .child(
        Row::new()
          .spacing(ExtraSpacing::Snug.role())
          .align_items(Alignment::Center)
          .child(icon(card.icon, IconSize::Md, glyph))
          .child(
            Text::new(&card.title)
              .id(format!("{}-title", card.id))
              .variant(ExtraTypography::OptionTitle.role()),
          ),
      )
      .child(Text::new(&card.description).variant(ExtraTypography::Prose.role()))
      .with_children(reason);
    let (fill, stroke) = if card.chosen {
      (PaletteColor::SurfaceRaised, PaletteColor::Accent)
    } else {
      (PaletteColor::SurfaceInput, PaletteColor::Border)
    };
    let frame = Button::empty()
      .id(card.id.clone())
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Start)
      .background(fill)
      .border(Border::inside(BorderSize::Sm, stroke))
      .rounded(RadiusSize::Md)
      .describe("chosen", if card.chosen { "true" } else { "false" })
      .describe("a11y_name", card.title.clone())
      .child(
        Column::new()
          .padding(Padding::new().top(ExtraSpacing::Hair.role()))
          .child(card.dot()),
      )
      .child(text);
    match card.choice {
      Choice::Choose(choose) => Focus::Form
        .apply(frame)
        .cursor(CursorIcon::Pointer)
        .on_click(move |event: MouseEvent| {
          event.stop_propagation();
          choose();
        })
        .into(),
      Choice::Unavailable(reason) => inert(frame.describe("reason", reason)).opacity(DISABLED_OPACITY).into(),
    }
  }
}
