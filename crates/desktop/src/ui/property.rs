//! Key/value property (`tfGRt`): a muted label and its value on one line.

use lurq::{
  app::theme::{PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, dimension::Dimension},
};

use crate::theme::ExtraTypography;

/// Row height (`tfGRt`).
const PROPERTY_HEIGHT: f32 = 20.0;

/// How a property value reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueStyle {
  Text,
  /// An identifier in mono: a remote, a branch, a path.
  Mono,
  /// A program or file path in the smaller mono of the connection inspectors (`bAHUI`, `DIs1e`).
  Path,
  /// Not known (yet), in the subtle color: *Checking…*, *Not signed in*.
  Pending,
  /// Not chosen or not set yet, in the muted color: *No pipeline chosen* (`zo13H` in `h3IIs`).
  Unset,
}

pub fn property(label: &str, value: &str) -> Element {
  styled_property(label, value, ValueStyle::Text)
}

pub fn styled_property(label: &str, value: &str, style: ValueStyle) -> Element {
  aligned_property(label, value, style, Alignment::Center)
}

/// The MCP details in 04.D (`QdLRd`) place text at the top of each 20px property row.
pub fn top_property(label: &str, value: &str, style: ValueStyle) -> Element {
  aligned_property(label, value, style, Alignment::Start)
}

/// Keep a property status visible when its preceding user value is too long for one row.
pub fn property_with_suffix(label: &str, value: &str, suffix: &str) -> Element {
  property_row(label, Alignment::Center)
    .height(Dimension::Px(PROPERTY_HEIGHT))
    .child(
      Row::new()
        .flex(1.0)
        .align_items(Alignment::Center)
        .justify(Justify::End)
        .child(
          Text::new(value)
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextPrimary)
            .nowrap()
            .text_overflow(TextOverflow::Elipsis)
            .text_align(TextAlign::Right)
            .flex(1.0)
            .selectable(true),
        )
        .child(
          Text::new(suffix)
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextPrimary)
            .nowrap()
            .flex_shrink(0.0)
            .selectable(true),
        ),
    )
    .into()
}

/// An MCP remote authentication value may span several lines (`J6IaSG` in `a4lm6D`).
pub fn wrapping_top_property(label: &str, value: &str) -> Element {
  property_row(label, Alignment::Start)
    .min_height(Dimension::Px(PROPERTY_HEIGHT))
    .child(
      Text::new(value)
        .variant(ExtraTypography::WrappingProperty.role())
        .color(PaletteColor::TextPrimary)
        .text_align(TextAlign::Right)
        .flex(1.0)
        .selectable(true),
    )
    .into()
}

/// A mono value shown in full, wrapping when it is longer than the row, e.g. a native connection's
/// endpoint (04.H4 `lJG42`).
pub fn wrapping_mono_property(label: &str, value: &str) -> Element {
  property_row(label, Alignment::Start)
    .min_height(Dimension::Px(PROPERTY_HEIGHT))
    .child(
      Text::new(value)
        .variant(ExtraTypography::MonoValue.role())
        .color(PaletteColor::TextPrimary)
        .text_align(TextAlign::Right)
        .flex(1.0)
        .selectable(true),
    )
    .into()
}

/// A mono value (`style` Mono) with a note under it that wraps, e.g. the code index an agent got and
/// why it had no ripgrep (`O4YLY` in `rd1Vo`).
pub fn noted_property(label: &str, value: &str, note: &str) -> Element {
  let value = Column::new()
    .flex(1.0)
    .align_items(Alignment::End)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(PROPERTY_HEIGHT))
        .align_items(Alignment::Center)
        .child(
          Text::new(value)
            .variant(ExtraTypography::MonoValue.role())
            .color(PaletteColor::TextPrimary)
            .nowrap()
            .text_overflow(TextOverflow::Elipsis)
            .text_align(TextAlign::Right)
            .flex(1.0)
            .selectable(true),
        ),
    )
    .child(
      Text::new(note)
        .variant(ExtraTypography::WrappingProperty.role())
        .color(PaletteColor::TextPrimary)
        .text_align(TextAlign::Right)
        .width(Dimension::Pct(100.0))
        .selectable(true),
    );
  property_row(label, Alignment::Start).child(value).into()
}

fn aligned_property(label: &str, value: &str, style: ValueStyle, alignment: Alignment) -> Element {
  let (typography, color) = match style {
    ValueStyle::Text => (TypographyStyle::Caption, PaletteColor::TextPrimary),
    ValueStyle::Mono => (ExtraTypography::MonoValue.role(), PaletteColor::TextPrimary),
    ValueStyle::Path => (TypographyStyle::Mono, PaletteColor::TextPrimary),
    ValueStyle::Pending => (TypographyStyle::Caption, PaletteColor::TextMuted),
    ValueStyle::Unset => (TypographyStyle::Caption, PaletteColor::TextSecondary),
  };
  property_row(label, alignment)
    .height(Dimension::Px(PROPERTY_HEIGHT))
    .child(
      Text::new(value)
        .variant(typography)
        .color(color)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .text_align(TextAlign::Right)
        .flex(1.0)
        .selectable(true),
    )
    .into()
}

fn property_row(label: &str, alignment: Alignment) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(alignment)
    .spacing(SpacingSize::Sm)
    .justify(Justify::SpaceBetween)
    // The label keeps its width; the value takes the rest after the gap.
    .child(
      Text::new(label)
        .variant(TypographyStyle::Caption)
        .nowrap()
        .flex_shrink(0.0),
    )
}
