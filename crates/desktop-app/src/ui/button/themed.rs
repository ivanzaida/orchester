//! A button drawn by lurq's `ui::Button` in the theme's recipe (`theme::kit::button`): its kind is the variant, its
//! height the size, its hold and its look the states. A select's label takes its room before its chevron
//! (`fill_label`); a stacked answer's consequence is the button's description, wrapping under its label. Its icons are the kit's, so the loader turns as it does
//! everywhere, under the ids `<id>-icon` and, as lurq names its parts, `<id>-label`.

use lurq::{
  app::theme::PaletteColor,
  node::Element,
  ui::{self, ButtonState, Styled},
};

use super::{ActionButton, ButtonKind, ButtonSize, Hold, state};
use crate::{
  theme::IconSize,
  ui::{
    icon::{Icon, icon},
    pressable::Focus,
  },
};

impl ActionButton {
  fn size(&self) -> ButtonSize {
    if self.in_heading {
      ButtonSize::Heading
    } else if self.compact {
      ButtonSize::Helper
    } else if self.small {
      ButtonSize::Small
    } else if self.snug {
      ButtonSize::Snug
    } else if self.pinned {
      ButtonSize::Pinned
    } else if self.large {
      ButtonSize::Large
    } else {
      ButtonSize::Regular
    }
  }

  /// The button with its look: kind, size, states and what it says to tooling.
  fn dressed(&self) -> ui::Button {
    let mut button = ui::Button::new(&self.label)
      .id(self.id.clone())
      .variant(self.kind.name())
      .size(self.size().name())
      .full_width(self.fill || self.stacked || self.kind == ButtonKind::Select)
      // A select's label takes its room, its chevron at the end.
      .fill_label(self.kind == ButtonKind::Select)
      .state(state::STACKED, self.stacked)
      .state(state::TIGHT, self.tight && !self.large)
      .state(state::ROOMY, self.roomy)
      .state(state::HELD, self.held)
      .state(state::MENU_OPEN, self.menu_open)
      .state(state::APPLIED, self.applied)
      .state(state::INVALID, self.invalid)
      .describe("kind", self.kind.name())
      .describe("state", self.state())
      .describe("a11y_name", self.label.clone());
    for (on, name) in [
      (self.held, state::HELD),
      (self.menu_open, state::MENU_OPEN),
      (self.applied, state::APPLIED),
      (self.invalid, state::INVALID),
    ] {
      if on {
        button = button.describe(name, "true");
      }
    }
    if let Some(Hold::Unavailable(reason) | Hold::UnavailableWithIcon(reason)) = &self.hold {
      button = button.describe("reason", reason.clone());
    }
    if let Some(consequence) = self.consequence.as_ref().filter(|_| self.stacked) {
      button = button.description(consequence).description_wraps(true);
    }
    button
  }

  pub(super) fn drawn(self) -> Element {
    let color = if self.applied {
      PaletteColor::Foreground
    } else {
      self.kind.content()
    };
    let (_, glyph, _) = self.metrics();
    let mut button = self.dressed();
    if let Some(lead) = self.lead() {
      button = button.leading(icon(lead, glyph, color.clone()).id(format!("{}-icon", self.id)));
    }
    if let Some(after) = self.after.filter(|_| self.hold.is_none()) {
      button = button.trailing(icon(after, glyph, color));
    }
    if self.kind == ButtonKind::Select {
      button = button.trailing(icon(Icon::ChevronDown, IconSize::Md, PaletteColor::MutedForeground));
    }
    if let Some(on_focus) = self.on_focus {
      button = button.on_focus(move || on_focus());
    }
    if let Some(element) = self.element {
      button = button.ref_element(element);
    }
    match (self.hold, self.action) {
      (None, Some(press)) => press.on_ui(button.tab_stop(self.focus != Focus::Pointer)).into(),
      (hold, _) => button
        .disabled(true)
        .state(ButtonState::Loading, hold == Some(Hold::Loading))
        .state(state::SHOWN, hold == Some(Hold::Shown))
        .into(),
    }
  }
}
