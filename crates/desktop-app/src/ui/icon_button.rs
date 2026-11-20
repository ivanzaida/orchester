//! Icon button (`Button/icon` `JJEzp`): a 16 `text_secondary` glyph in a 28 square, radius 6, the
//! `hover` fill under the pointer; the small one removes an entry from a row (`Remove` in `o6h0L5`, a 14
//! `text_muted` cross in a 24 square). It has no words of its own, so its label is its tooltip. lurq's `ui::Button`,
//! its icon alone, draws it in the theme's recipe (`theme::kit::icon_button`); the tooltip is the kit's.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  core::ElementRef,
  node::Element,
  ui::{self, Styled},
};

use super::{
  icon::Icon,
  pressable::{Focus, Press},
  tooltip::Tooltip,
};
use crate::{
  navigation::link::Open,
  theme::{ControlHeight, ExtraColor, IconSize},
};

/// The variant of lurq's `ui::Button` an icon button is in the theme's recipe.
pub const VARIANT: &str = "icon";
/// What it opened is open.
pub const ACTIVE: &str = "active";

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
  pub const ALL: [Self; 5] = [Self::Regular, Self::Small, Self::Inspector, Self::Compact, Self::Toggle];

  /// Its size in the theme's recipe of lurq's `ui::Button` (`theme::kit::icon_button`).
  pub const fn name(self) -> &'static str {
    match self {
      Self::Regular => "icon_regular",
      Self::Small => "icon_small",
      Self::Inspector => "icon_inspector",
      Self::Compact => "icon_compact",
      Self::Toggle => "icon_toggle",
    }
  }

  /// Its width and height.
  pub const fn sides(self) -> (ControlHeight, ControlHeight) {
    match self {
      Self::Regular => (ControlHeight::Md, ControlHeight::Md),
      Self::Small => (ControlHeight::Sm, ControlHeight::Sm),
      Self::Inspector => (ControlHeight::Inspector, ControlHeight::Inspector),
      Self::Compact => (ControlHeight::Xs, ControlHeight::Xs),
      Self::Toggle => (ControlHeight::Inspector, ControlHeight::Sm),
    }
  }

  /// The size and the color of its glyph.
  pub fn glyph(self) -> (IconSize, PaletteColor) {
    match self {
      Self::Regular => (IconSize::Lg, ExtraColor::TextSecondary.role()),
      Self::Small | Self::Compact | Self::Toggle => (IconSize::Md, PaletteColor::MutedForeground),
      Self::Inspector => (IconSize::Nav, ExtraColor::TextSecondary.role()),
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
  /// The keys that do the same, in its tooltip after the label.
  shortcut: Option<Arc<str>>,
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
      shortcut: None,
    }
  }

  /// The keys that do the same, after the label in its tooltip (`Search the conversation  Ctrl F`, `EGEVN` in 13). The
  /// longer tooltip lines up with the button's right edge, as it does for the search at the end of the page tabs.
  pub fn shortcut(mut self, keys: impl Into<Arc<str>>) -> Self {
    self.shortcut = Some(keys.into());
    self
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
    self.focus = Focus::Keyboard;
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let mut button = ui::Button::new(&self.label)
      .id(self.id.clone())
      .variant(VARIANT)
      .size(self.size.name())
      .icon(self.icon.name())
      .icon_only(true)
      .state(ACTIVE, self.active)
      .describe("a11y_name", self.label.clone());
    if let Some(element) = self.element {
      button = button.ref_element(element);
    }
    let button: Element = match self.action {
      Some(press) => press
        .on_ui(button.tab_stop(self.focus != Focus::Pointer))
        .describe("state", "enabled")
        .into(),
      None => button.disabled(true).describe("state", "disabled").into(),
    };
    if self.active {
      return button;
    }
    let tooltip = Tooltip::new(self.id, self.label);
    match self.shortcut {
      Some(keys) => tooltip.shortcut(keys).end().wrap(ctx, button),
      None => tooltip.wrap(ctx, button),
    }
  }
}
