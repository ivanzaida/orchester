//! The parts of an inspector beside a list (`Inspector · Claude · Kontur` `VIpsx`): a property row
//! (`Inspector row/value` `vhBEA`: what it is in 12/1.3 `text_secondary` in a 120 column and its value 8 after
//! it in `text_primary`, wrapping, at least 28 high), a section's title (`Inspector section title` `QDBns`:
//! 12/600 with an 11 `text_muted` count at its end), a note (`Inspector note` `vCguQ`: a 12 `text_muted` icon
//! and 11.5/1.4 `text_muted` words, 6 apart) and a link to what uses it (`Pipeline step link` `Q5bIlH`: a
//! 13 `text_muted` icon, its label in 12 and a 12 `arrow-up-right`, 8 apart, 10 at its sides, 28 high,
//! `surface_input` in a `border` stroke, radius 6).

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  icon::{Icon, icon},
  pressable::opening,
};
use crate::{
  navigation::link::Open,
  theme::{ControlHeight, ExtraSpacing, ExtraTypography, IconSize},
};

/// The column of what a property is (`Label` `X7na6R`).
const LABEL_WIDTH: f32 = 120.0;
/// Used-by link Q91kML.
const USED_BY_HEIGHT: f32 = 36.0;

/// How a property's value reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyValue {
  /// Words, in `text_primary`.
  Plain,
  /// A path, a command or an identifier, in Geist Mono (`Program file` in `VIpsx`).
  Mono,
  /// What is not known or not set, in `text_muted` (`Not checked yet` in `uBsQM`).
  Muted,
  /// What needs the person, in `warning` (`Not signed in` in `O1jDI`).
  Warning,
  /// What is wrong, in `danger` (`Not found` in `O1jDI`).
  Danger,
  /// A path that is wrong, in Geist Mono and `danger` (`C:\…\npm\claude.cmd` in `uBsQM`).
  MonoDanger,
}

/// A property with a value in words or code.
pub fn property(id: &str, label: &str, value: &str, kind: PropertyValue) -> Row {
  property_of(id, label, value_text(id, value, kind))
}

/// A property whose value is `value` as built.
pub fn property_of(id: &str, label: &str, value: impl Into<Element>) -> Row {
  framed(
    id,
    label,
    Text::new(label).variant(ExtraTypography::Property.role()),
    value,
  )
  .padding(Padding::vertical(ExtraSpacing::Snug.role()))
}

/// Compact editable property (LC20M), with the control owning its vertical inset.
pub fn control(id: &str, label: &str, value: impl Into<Element>) -> Row {
  framed(
    id,
    label,
    Text::new(label).variant(ExtraTypography::Property.role()),
    value,
  )
  .align_items(Alignment::Center)
}

fn value_text(id: &str, value: &str, kind: PropertyValue) -> Text {
  let (style, color) = match kind {
    PropertyValue::Plain => (ExtraTypography::Property.role(), PaletteColor::TextPrimary),
    PropertyValue::Mono => (ExtraTypography::PropertyMono.role(), PaletteColor::TextPrimary),
    PropertyValue::Muted => (ExtraTypography::Property.role(), PaletteColor::TextMuted),
    PropertyValue::Warning => (ExtraTypography::Property.role(), PaletteColor::Warning),
    PropertyValue::Danger => (ExtraTypography::Property.role(), PaletteColor::Danger),
    PropertyValue::MonoDanger => (ExtraTypography::PropertyMono.role(), PaletteColor::Danger),
  };
  Text::new(value)
    .id(format!("{id}-value"))
    .variant(style)
    .color(color)
    .width(Dimension::Pct(100.0))
    .min_width(Dimension::Px(0.0))
}

/// `label` in its column and `value` after it.
fn framed(id: &str, name: &str, label: Text, value: impl Into<Element>) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(ControlHeight::Md.value()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .describe("property", name.to_owned())
    .child(
      label
        .id(format!("{id}-label"))
        .width(Dimension::Px(LABEL_WIDTH))
        .flex_shrink(0.0),
    )
    .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(value))
}

/// A variable as a property: its name in Geist Mono 11.5 (`Variable · DOCS_INDEX` in `XRCed`), its value as
/// `kind` reads.
pub fn variable(id: &str, name: &str, value: &str, kind: PropertyValue) -> Row {
  let label = Text::new(name)
    .variant(ExtraTypography::MonoKey.role())
    .color(PaletteColor::TextSecondary);
  framed(id, name, label, value_text(id, value, kind)).padding(Padding::vertical(ExtraSpacing::Snug.role()))
}

/// A section's title with what it counts at its end, if anything.
pub fn section_title(id: &str, title: &str, meta: Option<&str>) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(title).variant(ExtraTypography::InspectorSection.role()))
    .child(Spacer::new().flex(1.0))
    .with_children(meta.map(|meta| {
      Text::new(meta)
        .id(format!("{id}-meta"))
        .variant(TypographyStyle::Caption)
    }))
}

/// A section: a line over it, 12 above its title, its parts `gap` apart.
pub fn section(id: &str, gap: SpacingSize) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Md))
    .spacing(gap)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
}

/// A note in muted words beside its icon.
pub fn note(id: &str, glyph: Icon, text: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(text)
        .variant(ExtraTypography::Note.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// A link to what uses the inspected item, which `open` opens; without one it only names it.
pub fn link(id: &str, glyph: Icon, label: &str, open: Option<Open>) -> Element {
  link_frame(id, glyph, label, open, false)
}

/// Secrets Used-by link (Q91kML), 36 high on the raised surface.
pub fn used_by_link(id: &str, glyph: Icon, label: &str, open: Option<Open>) -> Element {
  link_frame(id, glyph, label, open, true)
}

fn link_frame(id: &str, glyph: Icon, label: &str, open: Option<Open>, used_by: bool) -> Element {
  let content = |row: Row| {
    row
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(if used_by {
        USED_BY_HEIGHT
      } else {
        ControlHeight::Md.value()
      }))
      .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(if used_by {
        PaletteColor::SurfaceRaised
      } else {
        PaletteColor::SurfaceInput
      })
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .child(icon(
        glyph,
        if used_by { IconSize::Md } else { IconSize::Base },
        if used_by {
          PaletteColor::TextSecondary
        } else {
          PaletteColor::TextMuted
        },
      ))
      .child(
        Text::new(label)
          .variant(ExtraTypography::Property.role())
          .color(PaletteColor::TextPrimary)
          .flex(1.0)
          .nowrap(),
      )
  };
  match open {
    Some(open) => {
      let row = content(Row::new()).child(icon(Icon::ArrowUpRight, IconSize::Sm, PaletteColor::TextMuted));
      opening(open)
        .id(id.to_owned())
        .width(Dimension::Pct(100.0))
        .child(row)
        .into()
    }
    None => content(Row::new().id(id.to_owned())).into(),
  }
}
