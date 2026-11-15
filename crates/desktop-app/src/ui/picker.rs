//! Picker (`Picker` `ZQd2D`): a compact control that opens a menu of choices and shows the chosen one: a
//! 13 `text_secondary` icon, the choice in 12 `text_primary` and a 12 `text_muted` chevron, 6 apart and 8
//! from its sides, 26 high on `surface_input` in a `border` stroke, radius 6; the chevron points up while
//! its menu is open, its stroke `border_focus` (`LrZ5C` in `kadDh`). The caller places the menu at the picker through
//! the same `anchor`. A choice that needs attention takes a `warning` icon (`U34ugO` in `SyXwU`); one a form
//! needs and does not have, the `danger` outline and icon of a refused field.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::pressable,
};
use crate::theme::{ControlHeight, ExtraSpacing, IconSize};

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

impl From<Picker> for Element {
  fn from(picker: Picker) -> Self {
    let open = picker.open.clone();
    let is_open = picker.open.get();
    let glyph = match (picker.invalid, picker.warning) {
      (true, _) => PaletteColor::Danger,
      (false, true) => PaletteColor::Warning,
      (false, false) => PaletteColor::TextSecondary,
    };
    let outline = match (picker.invalid, is_open) {
      (true, _) => PaletteColor::Danger,
      (false, true) => PaletteColor::BorderFocus,
      (false, false) => PaletteColor::Border,
    };
    let trigger = pressable(move || open.update(|open| *open = !*open));
    let trigger = if picker.fill {
      trigger.width(Dimension::Pct(100.0))
    } else {
      trigger
    };
    trigger
      .id(picker.id.clone())
      .ref_element(picker.anchor)
      .height(Dimension::Px(ControlHeight::Picker.value()))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceInput)
      .border(Border::inside(BorderSize::Sm, outline))
      .rounded(RadiusSize::Md)
      .describe("a11y_name", picker.label.clone())
      .describe("state", if is_open { "open" } else { "closed" })
      .describe("invalid", picker.invalid.to_string())
      .child(icon(picker.glyph, IconSize::Base, glyph))
      .child({
        let label = Text::new(&picker.label)
          .id(format!("{}-label", picker.id))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextPrimary)
          .nowrap();
        if picker.fill {
          label
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .text_overflow(TextOverflow::Elipsis)
        } else {
          label
        }
      })
      .child(icon(
        if is_open { Icon::ChevronUp } else { Icon::ChevronDown },
        IconSize::Sm,
        PaletteColor::TextMuted,
      ))
      .into()
  }
}
