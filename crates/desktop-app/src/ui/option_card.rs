//! Option card (`Option card/off` `Imh8O`, `Option card/on` `g8tsS`): one choice of a few, as a card with its
//! radio dot, a 14 icon and its title in 12.5/500, and under them what it means in 12/1.45
//! `text_secondary`; 12 over and under, 14 at its sides, radius 6. Off it is `surface_input` in a `border`
//! stroke with an empty dot; chosen, `surface_raised` in an `accent` stroke with the dot filled. A choice that
//! cannot be made is faded, shows a lock in place of its dot and says why under its description (`Reason`
//! `a7DoR`); while the work it chose runs, it is faded with its dot kept. It is a form control: Tab reaches it
//! and Enter or Space chooses it.

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
  /// It cannot be changed while the work it chose runs.
  Waiting,
}

pub struct OptionCard {
  id: Arc<str>,
  icon: Icon,
  /// Shown in place of the icon.
  mark: Option<Element>,
  title: Arc<str>,
  description: Arc<str>,
  chosen: bool,
  choice: Choice,
  content: Vec<Element>,
  inline_summary: bool,
  /// The description is a path or a command: in mono.
  mono: bool,
  compact: bool,
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
      mark: None,
      title: title.into(),
      description: description.into(),
      chosen,
      content: Vec::new(),
      inline_summary: false,
      mono: false,
      compact: false,
      choice: Choice::Choose(Arc::new(choose)),
    }
  }

  /// A compact card, one of a few side by side (`Claude Code CLI` `qzI86` in `10 · Add provider`): no dot, its mark
  /// and title over one short line in 11.5 `text_muted`, 10 over and under and 12 at its sides; off it has no fill.
  /// One that cannot be chosen is faded to half and shows a lock at its title's end; why is its `reason`.
  pub fn compact(mut self) -> Self {
    self.compact = true;
    self
  }

  /// Additional details, such as starter steps or a source-version picker.
  pub fn content(mut self, content: impl Into<Element>) -> Self {
    self.content.push(content.into());
    self
  }

  /// Compact starting-point card: summary beside its title, details below (Iewpp).
  pub fn inline_summary(mut self) -> Self {
    self.inline_summary = true;
    self
  }

  /// The description is a path or a command, shown in mono (`A new folder Orchester creates` in `IOyJl`).
  pub fn mono_description(mut self) -> Self {
    self.mono = true;
    self
  }

  /// A mark in its own colors in place of the icon, e.g. the maker of an agent kind's models (`ui::agent_mark`).
  pub fn mark(mut self, mark: impl Into<Element>) -> Self {
    self.mark = Some(mark.into());
    self
  }

  fn text(&self, glyph: PaletteColor, mark: Option<Element>) -> Column {
    let lead = mark.unwrap_or_else(|| icon(self.icon, IconSize::Md, glyph).into());
    let title = Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .with_children((!self.inline_summary).then_some(lead))
      .child(
        Text::new(&self.title)
          .id(format!("{}-title", self.id))
          .variant(ExtraTypography::OptionTitle.role()),
      )
      .with_children(
        self
          .inline_summary
          .then(|| Text::new(&self.description).variant(ExtraTypography::Note.role())),
      );
    Column::new()
      .flex(1.0)
      .spacing(if self.inline_summary {
        SpacingSize::Sm
      } else {
        SpacingSize::Xs
      })
      .child(title)
      .with_children((!self.inline_summary).then(|| {
        Text::new(&self.description).variant(if self.mono {
          ExtraTypography::MonoKey.role()
        } else {
          ExtraTypography::Prose.role()
        })
      }))
  }

  /// The choice cannot be made, for `reason`.
  #[must_use]
  pub fn unavailable(mut self, reason: impl Into<Arc<str>>) -> Self {
    self.choice = Choice::Unavailable(reason.into());
    self
  }

  /// The choice cannot be changed while the work it chose runs: faded, its dot kept (`HKJj5` in `q7SPto`).
  #[must_use]
  pub fn waiting(mut self) -> Self {
    self.choice = Choice::Waiting;
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

impl OptionCard {
  /// The card as drawn, and how it can be chosen.
  fn frame(mut self) -> (Button, Choice) {
    let glyph = if self.chosen {
      PaletteColor::TextPrimary
    } else {
      PaletteColor::TextSecondary
    };
    let reason = match &self.choice {
      Choice::Unavailable(reason) => Some(
        Text::new(reason)
          .id(format!("{}-reason", self.id))
          .variant(ExtraTypography::Prose.role())
          .color(PaletteColor::TextMuted),
      ),
      Choice::Choose(_) | Choice::Waiting => None,
    };
    let mark = self.mark.take();
    let text = self.text(glyph, mark).with_children(reason);
    let (fill, stroke) = if self.chosen {
      (PaletteColor::SurfaceRaised, PaletteColor::Accent)
    } else {
      (PaletteColor::SurfaceInput, PaletteColor::Border)
    };
    let frame = Button::empty()
      .id(self.id.clone())
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Start)
      .background(fill)
      .border(Border::inside(BorderSize::Sm, stroke))
      .rounded(RadiusSize::Md)
      .describe("chosen", if self.chosen { "true" } else { "false" })
      .describe("a11y_name", self.title.clone())
      .child(
        Column::new()
          .padding(Padding::new().top(ExtraSpacing::Hair.role()))
          .child(self.dot()),
      )
      .child(text.with_children(self.content));
    (frame, self.choice)
  }

  /// The compact card as drawn, and how it can be chosen.
  fn compact_frame(mut self) -> (Button, Choice) {
    let lead = self
      .mark
      .take()
      .unwrap_or_else(|| icon(self.icon, IconSize::Md, PaletteColor::TextPrimary).into());
    let locked = matches!(self.choice, Choice::Unavailable(_));
    let head = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(lead)
      .child(
        Text::new(&self.title)
          .id(format!("{}-title", self.id))
          .variant(ExtraTypography::OptionTitle.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .nowrap(),
      )
      .with_children(locked.then(|| icon(Icon::Lock, IconSize::Sm, PaletteColor::TextMuted)));
    let text = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(head)
      .child(
        Text::new(&self.description)
          .id(format!("{}-detail", self.id))
          .variant(ExtraTypography::Note.role()),
      );
    let stroke = if self.chosen {
      PaletteColor::Accent
    } else {
      PaletteColor::Border
    };
    let frame = Button::empty()
      .id(self.id.clone())
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
      .align_items(Alignment::Start)
      .border(Border::inside(BorderSize::Sm, stroke))
      .rounded(RadiusSize::Md)
      .describe("chosen", if self.chosen { "true" } else { "false" })
      .describe("a11y_name", self.title.clone())
      .child(text);
    let frame = if self.chosen {
      frame.background(PaletteColor::SurfaceRaised)
    } else {
      frame
    };
    (frame, self.choice)
  }
}

impl From<OptionCard> for Element {
  fn from(card: OptionCard) -> Self {
    let (frame, choice) = if card.compact {
      card.compact_frame()
    } else {
      card.frame()
    };
    match choice {
      Choice::Choose(choose) => Focus::Keyboard
        .apply(frame)
        .cursor(CursorIcon::Pointer)
        .on_click(move |event: MouseEvent| {
          event.stop_propagation();
          choose();
        })
        .into(),
      // A card that cannot be chosen is faded to half, a compact one too (`HvtFb`, `osYDH` in `10` `ilfLu`).
      Choice::Unavailable(reason) => inert(frame.describe("reason", reason)).opacity(DISABLED_OPACITY).into(),
      Choice::Waiting => inert(frame).opacity(DISABLED_OPACITY).into(),
    }
  }
}
