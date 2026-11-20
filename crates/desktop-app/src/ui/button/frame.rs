//! The kit's own frame of a button, for what lurq's `ui::Button` does not draw (`super::themed`).

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Button, Column, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{ActionButton, ButtonKind, Hold};
use crate::{
  theme::{CONSEQUENCE_ON_ACCENT_OPACITY, DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::inert,
  },
};

impl ActionButton {
  /// The frame without its content: 28 high with its content centered, or stacked as wide as its parent, 8 over
  /// and under and 12 at the sides, its content at the start.
  fn frame(&self) -> Button {
    let (height, ..) = self.metrics();
    let frame = Button::empty()
      .id(self.id.clone())
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .describe("kind", self.kind.name())
      .describe("state", self.state())
      .describe("a11y_name", self.label.clone());
    let frame = if self.stacked {
      frame
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
        .spacing(SpacingSize::Md)
        .justify(Justify::Start)
    } else {
      frame
        .height(Dimension::Px(height.value()))
        .padding(Padding::horizontal(self.side()))
        .spacing(ExtraSpacing::Snug.role())
        .justify(Justify::Center)
    };
    let frame = if self.fill || self.kind == ButtonKind::Select {
      frame.width(Dimension::Pct(100.0))
    } else {
      frame
    };
    if self.bounded {
      return frame.max_width(Dimension::Pct(100.0)).min_width(Dimension::Px(0.0));
    }
    frame
  }

  /// The label in `color`; stacked, the label and the consequence under it.
  fn slot(&self, color: PaletteColor) -> Element {
    let (_, _, role) = self.metrics();
    let label = Text::new(&self.label)
      .id(format!("{}-label", self.id))
      .color(color)
      .nowrap();
    if self.stacked {
      let words = self.consequence.as_ref().map(|consequence| {
        let words = Text::new(consequence)
          .id(format!("{}-consequence", self.id))
          .variant(ExtraTypography::ButtonConsequence.role())
          .width(Dimension::Pct(100.0));
        match self.kind {
          ButtonKind::Primary => words
            .color(ExtraColor::OnAccent.role())
            .opacity(CONSEQUENCE_ON_ACCENT_OPACITY),
          _ => words,
        }
      });
      let text = Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(label.variant(ExtraTypography::ButtonStacked.role()))
        .with_children(words);
      return text.into();
    }
    let label = label.variant(role);
    let label = if self.bounded {
      label
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0))
        .text_overflow(lurq::components::TextOverflow::Elipsis)
    } else {
      label
    };
    let label = if self.kind == ButtonKind::Select {
      label.flex(1.0).min_width(Dimension::Px(0.0))
    } else {
      label
    };
    label.into()
  }
}

impl ActionButton {
  /// The kit's own frame: a select, a stacked answer and an answer that says what it does when focused, which lurq's
  /// button cannot draw (its description is one cut line, it has no focus handler, and its label does not grow).
  pub(super) fn framed(self) -> Element {
    let color = if self.applied {
      PaletteColor::Foreground
    } else {
      self.kind.content()
    };
    let (_, glyph, _) = self.metrics();
    let mut frame = self.frame();
    if let Some(lead) = self.lead() {
      frame = frame.child(icon(lead, glyph, color.clone()).id(format!("{}-icon", self.id)));
    }
    let after = self
      .after
      .filter(|_| self.hold.is_none())
      .map(|after| icon(after, glyph, color.clone()));
    frame = self.kind.dressed(frame).child(self.slot(color)).with_children(after);
    if self.kind == ButtonKind::Select {
      frame = frame.child(icon(Icon::ChevronDown, IconSize::Md, PaletteColor::MutedForeground));
    }
    if self.held {
      frame = frame.background(ExtraColor::Pressed.role()).describe("held", "true");
    }
    if self.menu_open {
      frame = frame
        .background(PaletteColor::Card)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Input))
        .describe("menu_open", "true");
    }
    if self.applied {
      frame = frame
        .background(PaletteColor::Highlight)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Input))
        .describe("applied", "true");
    }
    if self.invalid {
      frame = frame
        .border(Border::inside(BorderSize::Sm, PaletteColor::Destructive))
        .describe("invalid", "true");
    }
    if let Some(element) = self.element {
      frame = frame.ref_element(element);
    }
    if let Some(on_focus) = self.on_focus {
      frame = frame.on_focus(move || on_focus());
    }
    if let Some(Hold::Unavailable(reason) | Hold::UnavailableWithIcon(reason)) = &self.hold {
      frame = frame.describe("reason", reason.clone());
    }
    match (self.hold, self.action) {
      (None, Some(press)) => press
        .on(
          self
            .focus
            .apply(self.kind.interactive(frame))
            .cursor(CursorIcon::Pointer),
        )
        .into(),
      (Some(Hold::Loading), _) => inert(frame).cursor(CursorIcon::Progress).into(),
      (Some(Hold::Shown), _) => inert(frame).cursor(CursorIcon::Default).into(),
      _ => inert(frame).opacity(DISABLED_OPACITY).into(),
    }
  }
}
