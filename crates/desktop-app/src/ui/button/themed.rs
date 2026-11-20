//! A button drawn by lurq's `ui::Button` in the theme's recipe (`theme::kit::button`): its kind is the variant, its
//! height the size, its hold and its look the states. Its icons are the kit's, so the loader turns as it does
//! everywhere, under the ids `<id>-icon` and, as lurq names its parts, `<id>-label`.

use lurq::{
  app::theme::PaletteColor,
  node::Element,
  ui::{self, ButtonState, Styled},
};

use super::{ActionButton, ButtonKind, ButtonSize, Hold, state};
use crate::ui::{icon::icon, pressable::Focus};

impl ActionButton {
  /// Whether lurq's button draws it: all but a select, a stacked answer and one that says what it does when focused.
  pub(super) const fn themed(&self) -> bool {
    !self.stacked && self.on_focus.is_none() && !matches!(self.kind, ButtonKind::Select)
  }

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
      .full_width(self.fill)
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
