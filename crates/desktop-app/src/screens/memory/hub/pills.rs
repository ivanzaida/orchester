//! Status pills (`Status · Waiting for approval` `w3bOM` in Y1 `tsfxN`): a row of fully rounded pills, each a place of
//! the route with its name and count; the current one on `surface_raised` in a `border` stroke with its name in
//! 12/500 `text_primary`, the others without a fill or stroke, their names in 12 `text_secondary`; a count that waits
//! on the person in `warning`, the others `text_muted`.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, TypographyStyle},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};

use crate::{
  navigation::link::Open,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, HubText},
  ui::{
    pressable::{Press, pressed},
    skeleton,
  },
};

/// A pill's height (`w3bOM`).
const HEIGHT: f32 = 26.0;

pub(super) struct Pill {
  pub id: Arc<str>,
  pub label: Arc<str>,
  /// The count, and whether it waits on the person.
  pub count: Option<(usize, bool)>,
  /// The count is still read: its bone after the name.
  pub reading: bool,
  pub current: bool,
  pub open: Open,
}

impl From<Pill> for Element {
  fn from(pill: Pill) -> Self {
    let label = if pill.current {
      Text::new(&pill.label)
        .variant(TypographyStyle::Button)
        .color(PaletteColor::TextPrimary)
    } else {
      Text::new(&pill.label).variant(HubText::Status.role())
    }
    .nowrap();
    let count: Option<Element> = match pill.count.filter(|(count, _)| *count > 0) {
      Some((count, waits)) => Some(
        Text::new(&count.to_string())
          .id(format!("{}-count", pill.id))
          .variant(HubText::PanelHint.role())
          .color(if waits {
            PaletteColor::Warning
          } else {
            PaletteColor::TextMuted
          })
          .into(),
      ),
      None => pill
        .reading
        .then(|| skeleton::count().id(format!("{}-count", pill.id)).into()),
    };
    let row = Row::new()
      .height(Dimension::Px(HEIGHT))
      .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .rounded(ExtraRadius::Full.role())
      .child(label)
      .with_children(count);
    let row = if pill.current {
      row
        .background(PaletteColor::SurfaceRaised)
        .border_inside(BorderSize::Sm, PaletteColor::Border)
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
