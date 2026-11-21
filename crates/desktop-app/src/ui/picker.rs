//! Picker (`Picker` `ZQd2D`): a compact control that opens a menu of choices and shows the chosen one: a
//! 13 `text_secondary` icon, the choice in 12 `text_primary` and a 12 `text_muted` chevron, 6 apart and 8
//! from its sides, 26 high on `surface_input` in a `border` stroke, radius 6; the chevron points up while
//! its menu is open, its stroke `border_focus` (`LrZ5C` in `kadDh`). The caller places the menu at the picker through
//! the same `anchor`. A choice that needs attention takes a `warning` icon (`U34ugO` in `SyXwU`); one a form
//! needs and does not have, the `danger` outline and icon of a refused field. lurq's `ui::Button` draws it in the
//! variant [`VARIANT`]; one as wide as its room keeps the kit's frame, since lurq's label does not grow.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
  ui::{self, Styled},
};

use super::{
  button::state,
  icon::{Icon, icon},
  pressable::{Press, pressable},
};
use crate::theme::{ControlHeight, ExtraColor, ExtraSpacing, IconSize};

/// The variant of lurq's `ui::Button` a picker is in the theme's recipe (`theme::kit::trigger`).
pub const VARIANT: &str = "picker";
/// Its menu is open: the `border_focus` stroke.
pub const OPEN: &str = "open";

pub struct Picker {
  id: Arc<str>,
  glyph: Icon,
  label: Arc<str>,
  open: Signal<bool>,
  anchor: ElementRef,
  warning: bool,
  invalid: bool,
  fill: bool,
}

impl Picker {
  /// The picker `id` showing `glyph` and `label`, opening the menu `open` shows at `anchor`.
  pub fn new(
    id: impl Into<Arc<str>>,
    (glyph, label): (Icon, impl Into<Arc<str>>),
    open: Signal<bool>,
    anchor: ElementRef,
  ) -> Self {
    Self {
      id: id.into(),
      glyph,
      label: label.into(),
      open,
      anchor,
      warning: false,
      invalid: false,
      fill: false,
    }
  }

  /// The chosen value needs attention, e.g. a version that no longer exists.
  pub fn warning(mut self) -> Self {
    self.warning = true;
    self
  }
}

impl Picker {
  /// A choice the form needs and does not have: the `danger` outline and mark a refused field has.
  pub fn invalid(mut self) -> Self {
    self.invalid = true;
    self
  }

  /// As wide as the room it is given, the chevron at its end, the choice cut with an ellipsis where it does not fit:
  /// a form's field, where a picker in a toolbar is as wide as its words.
  pub fn fill(mut self) -> Self {
    self.fill = true;
    self
  }
}

impl Picker {
  /// The color of its icon: `danger` when refused, `warning` when it needs attention.
  fn glyph_color(&self) -> PaletteColor {
    match (self.invalid, self.warning) {
      (true, _) => PaletteColor::Destructive,
      (false, true) => PaletteColor::Warning,
      (false, false) => ExtraColor::TextSecondary.role(),
    }
  }

  fn chevron(is_open: bool) -> Text {
    icon(
      if is_open { Icon::ChevronUp } else { Icon::ChevronDown },
      IconSize::Sm,
      PaletteColor::MutedForeground,
    )
  }

  /// lurq's button in the variant [`VARIANT`]: a picker as wide as its words.
  fn drawn(self) -> Element {
    let open = self.open.clone();
    let is_open = self.open.get();
    let trigger = ui::Button::new(&self.label)
      .id(self.id.clone())
      .ref_element(self.anchor.clone())
      .variant(VARIANT)
      .tab_stop(false)
      .state(OPEN, is_open)
      .state(state::INVALID, self.invalid)
      .describe("a11y_name", self.label.clone())
      .describe("state", if is_open { "open" } else { "closed" })
      .describe("invalid", self.invalid.to_string())
      .leading(icon(self.glyph, IconSize::Base, self.glyph_color()))
      .trailing(Self::chevron(is_open));
    Press::run(move || open.update(|open| *open = !*open))
      .on_ui(trigger)
      .into()
  }

  /// The kit's own frame of a picker as wide as its room, whose choice takes the room up to the chevron at its end:
  /// lurq's button keeps its label as wide as its words.
  fn framed(self) -> Element {
    let open = self.open.clone();
    let is_open = self.open.get();
    let outline = match (self.invalid, is_open) {
      (true, _) => PaletteColor::Destructive,
      (false, true) => PaletteColor::Ring,
      (false, false) => PaletteColor::Border,
    };
    pressable(move || open.update(|open| *open = !*open))
      .width(Dimension::Pct(100.0))
      .id(self.id.clone())
      .ref_element(self.anchor.clone())
      .height(Dimension::Px(ControlHeight::Picker.value()))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .background(ExtraColor::InputSurface.role())
      .border(Border::inside(BorderSize::Sm, outline))
      .rounded(RadiusSize::Md)
      .describe("a11y_name", self.label.clone())
      .describe("state", if is_open { "open" } else { "closed" })
      .describe("invalid", self.invalid.to_string())
      .child(icon(self.glyph, IconSize::Base, self.glyph_color()))
      .child(
        Text::new(&self.label)
          .id(format!("{}-label", self.id))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::Foreground)
          .nowrap()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .text_overflow(TextOverflow::Elipsis),
      )
      .child(Self::chevron(is_open))
      .into()
  }
}

impl From<Picker> for Element {
  fn from(picker: Picker) -> Self {
    if picker.fill { picker.framed() } else { picker.drawn() }
  }
}
