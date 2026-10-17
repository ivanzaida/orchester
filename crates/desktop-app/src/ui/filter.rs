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

/// A filter that is on or off, as a filter button that opens no menu (`Not shared · 2` in `Xr4Wb`, on in `A7PKfL`):
/// on, it has a cross in place of the chevron; a press turns it on or off.
pub struct FilterToggle {
  id: Arc<str>,
  label: Arc<str>,
  on: bool,
  press: Arc<dyn Fn() + Send + Sync>,
}

impl FilterToggle {
  /// The toggle `id` saying `label`, `on` or not; `press` turns it the other way.
  pub fn new(
    id: impl Into<Arc<str>>,
    label: impl Into<Arc<str>>,
    on: bool,
    press: impl Fn() + Send + Sync + 'static,
  ) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      on,
      press: Arc::new(press),
    }
  }
}

impl From<FilterToggle> for Element {
  fn from(toggle: FilterToggle) -> Self {
    let (fill, stroke, text, glyph) = if toggle.on {
      (
        PaletteColor::AccentMuted,
        PaletteColor::BorderStrong,
        PaletteColor::TextPrimary,
        Some(icon(Icon::X, IconSize::Base, PaletteColor::TextSecondary)),
      )
    } else {
      (
        PaletteColor::SurfaceInput,
        PaletteColor::Border,
        PaletteColor::TextSecondary,
        None,
      )
    };
    let press = toggle.press.clone();
    pressable(move || press())
      .id(toggle.id.clone())
      .height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::new().left(ExtraSpacing::Loose.role()).right(if toggle.on {
        SpacingSize::Sm
      } else {
        ExtraSpacing::Loose.role()
      }))
      .spacing(ExtraSpacing::Snug.role())
      .background(fill)
      .border(Border::inside(BorderSize::Sm, stroke))
      .rounded(RadiusSize::Md)
      .hovered_style(Style::new().border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong)))
      .describe("a11y_name", toggle.label.clone())
      .describe("active", if toggle.on { "true" } else { "false" })
      .child(
        Text::new(&toggle.label)
          .id(format!("{}-label", toggle.id))
          .variant(TypographyStyle::Label)
          .color(text)
          .nowrap(),
      )
      .with_children(glyph)
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

/// A filter's option `id` saying `label`, `chosen` or not, with its value's swatch where it has one (`u1ZxRL`);
/// a press runs `toggle`, and the menu stays open. A `mono` label is an identifier, such as a model, in mono 11
/// (`claude-opus-5-5` `aQCL8`).
pub fn filter_option(
  id: impl Into<Arc<str>>,
  (label, chosen, mono): (&str, bool, bool),
  swatch: Option<Element>,
  toggle: impl Fn() + Send + Sync + 'static,
) -> Element {
  let id = id.into();
  let text = Text::new(label)
    .id(format!("{id}-label"))
    .variant(if mono {
      ExtraTypography::MonoNote.role()
    } else {
      TypographyStyle::Description
    })
    .color(PaletteColor::TextPrimary)
    .nowrap()
    .flex(1.0)
    .min_width(Dimension::Px(0.0));
  option_frame(&id, (label, chosen), swatch, toggle)
    .height(Dimension::Px(ControlHeight::Md.value()))
    .child(text)
    .into()
}

/// An option that says under its label what it chooses (*No account* in `EU0ex`), as tall as its words, its
/// check and swatch beside the label's line.
pub fn filter_option_explained(
  id: impl Into<Arc<str>>,
  (label, detail, chosen): (&str, &str, bool),
  swatch: Option<Element>,
  toggle: impl Fn() + Send + Sync + 'static,
) -> Element {
  let id = id.into();
  let words = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(label)
        .id(format!("{id}-label"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary),
    )
    .child(
      Text::new(detail)
        .id(format!("{id}-detail"))
        .width(Dimension::Pct(100.0))
        .variant(TypographyStyle::Caption),
    );
  option_frame(&id, (label, chosen), swatch, toggle)
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
    .align_items(Alignment::Start)
    .child(words)
    .into()
}

/// The heading of a group of options (`Group · Codex` `ezGJl`): its icon and name in 11/500 `text_muted`,
/// 24 high.
pub fn filter_group(id: impl Into<Arc<str>>, glyph: Icon, label: &str) -> Element {
  Row::new()
    .id(id.into())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(GROUP_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(label)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

/// An option's pressable row with its check and swatch; its words go after them.
fn option_frame(
  id: &Arc<str>,
  (label, chosen): (&str, bool),
  swatch: Option<Element>,
  toggle: impl Fn() + Send + Sync + 'static,
) -> lurq::components::Button {
  pressable(toggle)
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .describe("a11y_name", label.to_owned())
    .describe("checked", if chosen { "true" } else { "false" })
    .child(check_mark(chosen))
    .with_children(swatch.map(|swatch| {
      Row::new()
        .height(Dimension::Px(SWATCH_BOX))
        .flex_shrink(0.0)
        .align_items(Alignment::Center)
        .child(swatch)
    }))
}

/// A group heading's height (`ezGJl`), and the box an option's swatch sits in (`Swatch box` `eOYES`).
const GROUP_HEIGHT: f32 = 24.0;
const SWATCH_BOX: f32 = 16.0;
