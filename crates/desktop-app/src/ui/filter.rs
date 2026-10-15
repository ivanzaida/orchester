//! Filters. A filter button (`Filter button/default` `J7Tb6n`) opens a filter's menu: its name in 12/500
//! `text_secondary` and a 13 `text_muted` chevron, 6 apart, 10 before and 8 after them, 28 high on
//! `surface_input` inside a `border` stroke, radius 6; while the filter chooses something
//! (`Filter button/active` `q4oYbV`) it says how much (*Model · 1 of 5*) in `text_primary` with a
//! `text_secondary` chevron, on `accent_muted` inside a `border_strong` stroke. A filter chip (`Filter chip`
//! `zjUM8`) names one chosen value: the filter in 11.5 `text_muted`, the value in 11.5/500 `text_primary`
//! and a 12 `text_muted` cross that lets it go, 5 apart, 9 before and 6 after them, 24 high on
//! `surface_raised` inside a `border` stroke, fully rounded. A filter's menu (`CLI menu` `tEymU`) is
//! `surface_raised` inside a `border_strong` stroke, radius 10, 6 inside and 2 between its options, lifted by
//! the `lg` shadow; an option is 28 high, radius 4, 8 at its sides: a checkbox, an 8 swatch where the
//! value has a color, and the value in 12 `text_primary`, 8 apart, the `hover` fill under the pointer.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize, TypographyStyle},
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::pressable,
  toggle::check_mark,
};
use crate::theme::{ControlHeight, ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize};

pub struct FilterButton {
  id: Arc<str>,
  label: Arc<str>,
  active: bool,
  open: Signal<bool>,
  anchor: ElementRef,
}

impl FilterButton {
  /// The button `id` saying `label`, which opens the menu `open` shows at `anchor`.
  pub fn new(id: impl Into<Arc<str>>, label: impl Into<Arc<str>>, open: Signal<bool>, anchor: ElementRef) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      active: false,
      open,
      anchor,
    }
  }

  /// The filter chooses something.
  #[must_use]
  pub const fn active(mut self, active: bool) -> Self {
    self.active = active;
    self
  }
}

impl From<FilterButton> for Element {
  fn from(button: FilterButton) -> Self {
    let (fill, stroke, text, chevron) = if button.active {
      (
        PaletteColor::AccentMuted,
        PaletteColor::BorderStrong,
        PaletteColor::TextPrimary,
        PaletteColor::TextSecondary,
      )
    } else {
      (
        PaletteColor::SurfaceInput,
        PaletteColor::Border,
        PaletteColor::TextSecondary,
        PaletteColor::TextMuted,
      )
    };
    let open = button.open.clone();
    pressable(move || open.update(|open| *open = !*open))
      .id(button.id.clone())
      .ref_element(button.anchor)
      .height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::new().left(ExtraSpacing::Loose.role()).right(SpacingSize::Sm))
      .spacing(ExtraSpacing::Snug.role())
      .background(fill)
      .border(Border::inside(BorderSize::Sm, stroke))
      .rounded(RadiusSize::Md)
      .hovered_style(Style::new().border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong)))
      .describe("a11y_name", button.label.clone())
      .describe("active", if button.active { "true" } else { "false" })
      .child(
        Text::new(&button.label)
          .id(format!("{}-label", button.id))
          .variant(TypographyStyle::Label)
          .color(text)
          .nowrap(),
      )
      .child(icon(Icon::ChevronDown, IconSize::Base, chevron))
      .into()
  }
}

pub struct FilterChip {
  id: Arc<str>,
  facet: Arc<str>,
  value: Arc<str>,
  /// What tools call the cross, and what a press does.
  remove: (Arc<str>, Arc<dyn Fn() + Send + Sync>),
}

impl FilterChip {
  /// The chip `id` naming `value` of `facet`; its cross, named `label`, runs `remove`.
  pub fn new(
    id: impl Into<Arc<str>>,
    (facet, value): (impl Into<Arc<str>>, impl Into<Arc<str>>),
    label: impl Into<Arc<str>>,
    remove: impl Fn() + Send + Sync + 'static,
  ) -> Self {
    Self {
      id: id.into(),
      facet: facet.into(),
      value: value.into(),
      remove: (label.into(), Arc::new(remove)),
    }
  }
}

impl From<FilterChip> for Element {
  fn from(chip: FilterChip) -> Self {
    let (label, remove) = chip.remove;
    let cross = pressable(move || remove())
      .id(format!("{}-remove", chip.id))
      .describe("a11y_name", label)
      .rounded(ExtraRadius::Full.role())
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(icon(Icon::X, IconSize::Sm, PaletteColor::TextMuted));
    Row::new()
      .id(chip.id.clone())
      .height(Dimension::Px(ControlHeight::Sm.value()))
      .padding(
        Padding::new()
          .left(ExtraSpacing::ChipStart.role())
          .right(ExtraSpacing::Snug.role()),
      )
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(ExtraRadius::Full.role())
      .child(Text::new(&chip.facet).variant(ExtraTypography::Note.role()).nowrap())
      .child(
        Text::new(&chip.value)
          .id(format!("{}-value", chip.id))
          .variant(ExtraTypography::NoteStrong.role())
          .nowrap(),
      )
      .child(cross)
      .into()
  }
}

/// The panel of a filter's menu, `width` wide, with the element id `id`; add its options as children.
pub fn filter_menu_panel(id: &str, width: f32) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Px(width))
    .padding(ExtraSpacing::Snug.role())
    .spacing(ExtraSpacing::Tight.role())
    .rounded(RadiusSize::Lg)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
    .box_shadow(ShadowStyle::Lg)
}

/// A filter's option `id` saying `label`, `chosen` or not, with its value's color as a swatch where it has
/// one; a press runs `toggle`, and the menu stays open. A `mono` label is an identifier, such as a model, in
/// mono 11 (`claude-opus-5-5` `aQCL8`).
pub fn filter_option(
  id: impl Into<Arc<str>>,
  (label, chosen, mono): (&str, bool, bool),
  swatch: Option<PaletteColor>,
  toggle: impl Fn() + Send + Sync + 'static,
) -> Element {
  let id = id.into();
  pressable(toggle)
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ControlHeight::Md.value()))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .describe("a11y_name", label.to_owned())
    .describe("checked", if chosen { "true" } else { "false" })
    .child(check_mark(chosen))
    .with_children(swatch.map(|color| {
      Row::new()
        .width(Dimension::Px(SWATCH))
        .height(Dimension::Px(SWATCH))
        .flex_shrink(0.0)
        .background(color)
        .rounded(ExtraRadius::Swatch.role())
    }))
    .child(
      Text::new(label)
        .id(format!("{id}-label"))
        .variant(if mono {
          ExtraTypography::MonoNote.role()
        } else {
          TypographyStyle::Description
        })
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// A value's color swatch in an option (`u1ZxRL`).
const SWATCH: f32 = 8.0;
