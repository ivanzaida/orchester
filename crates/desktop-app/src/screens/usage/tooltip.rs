//! A chart's tooltip (`Chart tooltip` `G7cuH`): 216 wide on `surface_raised` inside a `border_strong`
//! stroke, radius 8, lifted by the `lg` shadow, 10 over and under its content and 12 at its sides, its
//! parts 8 apart: the title in 12/600, lines 5 apart (an 8 swatch where the line is a series, the name in 12
//! `text_secondary`, the exact figure in 12 mono at the end), and where it came from in 11 `text_muted`. A
//! reset marker's tooltip is its title alone on one line, 282 wide (`Tooltip · reset` `K0L758`). It sits
//! beside what it describes, on its left where the right has no room.

use lurq::{
  app::theme::{BorderSize, PaletteColor, ShadowStyle, SpacingSize, TypographyStyle},
  components::{Column, Placement, Popup, Row, Text},
  core::ElementRef,
  layout::Alignment,
  node::{Element, HitTestBehavior, dimension::Dimension, padding::Padding},
};

use super::card::SWATCH;
use crate::theme::{ExtraRadius, ExtraSpacing, ExtraTypography};

/// The tooltip's width, a reset marker's, and how far it keeps from what it describes.
pub const WIDTH: f32 = 216.0;
pub const TITLE_WIDTH: f32 = 282.0;
const INFO_WIDTH: f32 = 264.0;
const OFFSET: f32 = 12.0;

/// The tooltip `id`: `title`, its `lines`, and where it came from.
pub fn tip(id: &str, title: &str, lines: Vec<Element>, meta: Option<&str>) -> Element {
  frame(id, WIDTH)
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(ExtraTypography::InspectorSection.role()),
    )
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Pair.role())
        .with_children(lines),
    )
    .with_children(meta.map(|meta| {
      Text::new(meta)
        .id(format!("{id}-meta"))
        .width(Dimension::Pct(100.0))
        .variant(TypographyStyle::Caption)
    }))
    .into()
}

/// The tooltip `id` that is its `title` alone, on one line.
pub fn title_tip(id: &str, title: &str) -> Element {
  frame(id, TITLE_WIDTH)
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(ExtraTypography::InspectorSection.role())
        .nowrap(),
    )
    .into()
}

fn frame(id: &str, width: f32) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Px(width))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
    .rounded(ExtraRadius::Tile.role())
    .box_shadow(ShadowStyle::Lg)
}

/// One line: a swatch where it is a series (the room of one where it is not), its name, its figure if any.
pub fn line(mark: Option<Element>, name: &str, value: Option<&str>) -> Element {
  let mark = mark.unwrap_or_else(|| Row::new().width(Dimension::Px(SWATCH)).flex_shrink(0.0).into());
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(mark)
    .child(
      Text::new(name)
        .variant(TypographyStyle::Description)
        .nowrap()
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(value.map(|value| Text::new(value).variant(TypographyStyle::Mono).nowrap()))
    .into()
}

/// A line under a series' line that names one of the accounts it holds, muted and smaller (*Claude · Personal*
/// under *Other accounts · 2* in `EA7QF`).
pub fn detail_line(name: &str, value: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(SWATCH + ExtraSpacing::Snug.value()))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(name)
        .variant(TypographyStyle::Caption)
        .nowrap()
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      Text::new(value)
        .variant(ExtraTypography::MonoNote.role())
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

/// A tooltip that says what a mark means (`Tooltip/no account` `X5TNDZ`): its swatch and title over the
/// words, 264 wide.
pub fn info_tip(id: &str, (mark, title): (Element, &str), body: &str) -> Element {
  frame(id, INFO_WIDTH)
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(mark)
        .child(
          Text::new(title)
            .id(format!("{id}-title"))
            .variant(ExtraTypography::InspectorSection.role()),
        ),
    )
    .child(
      Text::new(body)
        .id(format!("{id}-body"))
        .width(Dimension::Pct(100.0))
        .variant(TypographyStyle::Description),
    )
    .into()
}

/// `tip`, `tip_width` wide, placed in a chart `width` wide beside what spans `left` to `right`: after it,
/// or before it when the room after it is too narrow.
/// It opens over the page from the chart's top (`chart`, `height` high), so a long tooltip runs past the card
/// rather than being cut by it (`EA7QF`).
pub fn beside(
  (chart, height): (ElementRef, f32),
  (tip, tip_width): (Element, f32),
  (left, right): (f32, f32),
  width: f32,
) -> Element {
  let after = right + OFFSET;
  let x = if after + tip_width <= width {
    after
  } else {
    (left - OFFSET - tip_width).max(0.0)
  };
  Popup::new(chart, tip)
    .open_when(true)
    .placement(Placement::BottomStart)
    .offset(x, -height)
    .hit_test(HitTestBehavior::None)
    .dismiss_on_outside_click(false)
    .dismiss_on_escape(false)
    .into()
}
