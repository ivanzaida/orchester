//! Icon button (`Button/icon` `JJEzp`): a 16 `text_secondary` glyph in a 28 square, radius 6, the
//! `hover` fill under the pointer; the small one removes an entry from a row (`Remove` in `o6h0L5`, a 14
//! `text_muted` cross in a 24 square). It has no words of its own, so its label is its tooltip.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::mouse_event::MouseEvent,
    theme::{PaletteColor, RadiusSize},
  },
  components::Button,
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, dimension::Dimension},
};

use super::{
  icon::{Icon, icon},
  pressable::{Focus, inert},
  tooltip::Tooltip,
};
use crate::theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, IconSize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconButtonSize {
  /// 28 square, 16 `text_secondary` glyph.
  Regular,
  /// 24 square, 14 `text_muted` glyph: removing an entry from a row.
  Small,
}

impl IconButtonSize {
  const fn side(self) -> f32 {
    match self {
      Self::Regular => ControlHeight::Md.value(),
      Self::Small => ControlHeight::Sm.value(),
    }
  }

  const fn glyph(self) -> (IconSize, PaletteColor) {
    match self {
      Self::Regular => (IconSize::Lg, PaletteColor::TextSecondary),
      Self::Small => (IconSize::Md, PaletteColor::TextMuted),
    }
  }
}

pub struct IconButton {
  id: Arc<str>,
  icon: Icon,
  label: Arc<str>,
  size: IconButtonSize,
  action: Option<Arc<dyn Fn() + Send + Sync>>,
  focus: Focus,
}

impl IconButton {
  /// `label` says what the button does; it is the tooltip and the name tools read. Without
  /// [`Self::on_click`] the button is faded and cannot be pressed.
  pub fn new(id: impl Into<Arc<str>>, icon: Icon, label: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      icon,
      label: label.into(),
      size: IconButtonSize::Regular,
      action: None,
      focus: Focus::Pointer,
    }
  }

  pub fn small(mut self) -> Self {
    self.size = IconButtonSize::Small;
    self
  }

  pub fn on_click(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.action = Some(Arc::new(handler));
    self
  }

  /// A button of a form: a Tab stop with the focus ring.
  pub fn in_form(mut self) -> Self {
    self.focus = Focus::Form;
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let side = Dimension::Px(self.size.side());
    let (size, color) = self.size.glyph();
    let button = Button::empty()
      .id(self.id.clone())
      .width(side)
      .height(side)
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .rounded(RadiusSize::Md)
      .describe("label", self.label.clone())
      .child(icon(self.icon, size, color).id(format!("{}-icon", self.id)));
    let button = match self.action {
      Some(handler) => self
        .focus
        .apply(button)
        .cursor(CursorIcon::Pointer)
        .hovered_style(Style::new().background(ExtraColor::Hover.role()))
        .active_style(Style::new().background(ExtraColor::Pressed.role()))
        .describe("state", "enabled")
        .on_click(move |event: MouseEvent| {
          event.stop_propagation();
          handler();
        }),
      None => inert(button).opacity(DISABLED_OPACITY).describe("state", "disabled"),
    };
    Tooltip::new(self.id, self.label).wrap(ctx, button)
  }
}
