//! Status pills (`Pills` in 05): a row of fully rounded pills, each a place of the route with its name and count; the
//! current one raised and outlined, a count that waits on the person in `warning`.
use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use crate::{
  navigation::link::Open,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, HubText},
  ui::pressable::{Press, pressed},
};

/// A pill's height (`Pill` `TVWsV`).
const HEIGHT: f32 = 26.0;

pub(crate) struct Pill {
  pub id: Arc<str>,
  pub label: Arc<str>,
  /// The count, and whether it waits on the person.
  pub count: Option<(usize, bool)>,
  pub current: bool,
  pub open: Open,
}

impl From<Pill> for Element {
  fn from(pill: Pill) -> Self {
    let label = Text::new(&pill.label)
      .variant(HubText::BarStatus.role())
      .color(if pill.current {
        PaletteColor::TextPrimary
      } else {
        PaletteColor::TextSecondary
      })
      .nowrap();
    let count = pill.count.filter(|(count, _)| *count > 0).map(|(count, waits)| {
      Text::new(&count.to_string())
        .id(format!("{}-count", pill.id))
        .variant(HubText::Hint.role())
        .color(if waits {
          PaletteColor::Warning
        } else {
          PaletteColor::TextMuted
        })
    });
    let row = Row::new()
      .height(Dimension::Px(HEIGHT))
      .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .rounded(ExtraRadius::Full.role())
      .border_inside(
        BorderSize::Sm,
        if pill.current {
          PaletteColor::BorderStrong
        } else {
          PaletteColor::Border
        },
      )
      .child(label)
      .with_children(count);
    let row = if pill.current {
      row.background(PaletteColor::SurfaceRaised)
    } else {
      row
    };
    pressed(Press::Open(pill.open))
      .id(pill.id.clone())
      .describe("current", pill.current.to_string())
      .rounded(ExtraRadius::Full.role())
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(row)
      .into()
  }
}

/// `pills` in a row, then `end` at its right.
pub(crate) fn strip(id: &str, pills: Vec<Pill>, end: Vec<Pill>) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .with_children(pills.into_iter().map(Element::from))
    .child(Row::new().flex(1.0))
    .with_children(end.into_iter().map(Element::from))
}
