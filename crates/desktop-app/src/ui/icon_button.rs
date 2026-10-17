//! Icon button (`Button/icon` `JJEzp`): a 16 `text_secondary` glyph in a 28 square, radius 6, the
//! `hover` fill under the pointer; the small one removes an entry from a row (`Remove` in `o6h0L5`, a 14
//! `text_muted` cross in a 24 square). It has no words of its own, so its label is its tooltip.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize},
  },
  components::Button,
  core::ElementRef,
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, dimension::Dimension},
};

use super::{
  icon::{Icon, icon},
  pressable::{Focus, Press, inert},
  tooltip::Tooltip,
};
use crate::{
  navigation::link::Open,
  theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, IconSize},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconButtonSize {
  /// 28 square, 16 `text_secondary` glyph.
  Regular,
  /// 24 square, 14 `text_muted` glyph: removing an entry from a row.
  Small,
  /// Inspector close (Secrets m1So5): 26 square, 15 glyph.
  Inspector,
  /// 20 square, 14 `text_muted` glyph: a panel head's action (`Collapse all` `mJGHi` in `o1Uea4`).
  Compact,
  /// 26 wide and 24 high, 14 `text_muted` glyph: a toggle among small buttons (`Match case` `O7iJW` in
  /// `b7RGL`).
  Toggle,
}

impl IconButtonSize {
  /// Its width and height.
  const fn sides(self) -> (f32, f32) {
    match self {
      Self::Regular => (ControlHeight::Md.value(), ControlHeight::Md.value()),
      Self::Small => (ControlHeight::Sm.value(), ControlHeight::Sm.value()),
      Self::Inspector => (ControlHeight::Inspector.value(), ControlHeight::Inspector.value()),
      Self::Compact => (ControlHeight::Xs.value(), ControlHeight::Xs.value()),
      Self::Toggle => (ControlHeight::Inspector.value(), ControlHeight::Sm.value()),
    }
  }

  const fn glyph(self) -> (IconSize, PaletteColor) {
    match self {
      Self::Regular => (IconSize::Lg, PaletteColor::TextSecondary),
      Self::Small | Self::Compact | Self::Toggle => (IconSize::Md, PaletteColor::TextMuted),
      Self::Inspector => (IconSize::Nav, PaletteColor::TextSecondary),
    }
  }
}

pub struct IconButton {
  id: Arc<str>,
  icon: Icon,
  label: Arc<str>,
  size: IconButtonSize,
  action: Option<Press>,
  focus: Focus,
  active: bool,
  element: Option<ElementRef>,
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
      active: false,
      element: None,
    }
  }

  pub fn small(mut self) -> Self {
    self.size = IconButtonSize::Small;
    self
  }

  pub fn compact(mut self) -> Self {
    self.size = IconButtonSize::Compact;
    self
  }

  pub fn toggle(mut self) -> Self {
    self.size = IconButtonSize::Toggle;
    self
  }

  pub fn inspector(mut self) -> Self {
    self.size = IconButtonSize::Inspector;
    self
  }

  pub fn on_click(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.action = Some(Press::run(handler));
    self
  }

  /// A button that navigates: a press opens its place with `open` by a browser's rules (`navigation::link`).
  pub fn opens(mut self, open: Open) -> Self {
    self.action = Some(Press::Open(open));
    self
  }

  /// What it opened is open: `pressed` fill and a `text_primary` glyph (`Remove` `rDFim` in `mykcc`), and no
  /// tooltip, which would cover what it opened (`Warning threshold` popover `o4ql9`).
  pub fn active(mut self) -> Self {
    self.active = true;
    self
  }

  /// The button's element, for a popup it opens to anchor to: lurq switches to that popup in one press, from
  /// another open dropdown, only when the pressed button is the popup's anchor itself.
  pub fn element(mut self, element: ElementRef) -> Self {
    self.element = Some(element);
    self
  }

  /// A button of a form: a Tab stop with the focus ring.
  pub fn in_form(mut self) -> Self {
    self.focus = Focus::Form;
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let (width, height) = self.size.sides();
    let (size, color) = self.size.glyph();
    let color = if self.active { PaletteColor::TextPrimary } else { color };
    let button = Button::empty()
      .id(self.id.clone())
      .width(Dimension::Px(width))
      .height(Dimension::Px(height))
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .rounded(RadiusSize::Md)
      .describe("a11y_name", self.label.clone())
      .child(icon(self.icon, size, color).id(format!("{}-icon", self.id)));
    let button = if self.active {
      button.background(ExtraColor::Pressed.role())
    } else {
      button
    };
    let button = match self.element {
      Some(element) => button.ref_element(element),
      None => button,
    };
    let button = match self.action {
      Some(press) => press.on(
        self
          .focus
          .apply(button)
          .cursor(CursorIcon::Pointer)
          .hovered_style(Style::new().background(ExtraColor::Hover.role()))
          .active_style(Style::new().background(ExtraColor::Pressed.role()))
          .describe("state", "enabled"),
      ),
      None => inert(button).opacity(DISABLED_OPACITY).describe("state", "disabled"),
    };
    if self.active {
      return button.into();
    }
    Tooltip::new(self.id, self.label).wrap(ctx, button)
  }
}
