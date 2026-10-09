//! The parts every startup screen is made of (`g0Kj1`, `kMM56`): the icon tile, the heading with what
//! failed, what to do, the actions and a footnote. Keys and values, the path with Copy and the buttons
//! are the UI kit's (`ui::detail_row`, `ui::code_box`, `ui::button`).

use std::{path::PathBuf, thread};

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_exec::external;

use crate::{
  theme::{ExtraSpacing, ExtraTypography, StartupText},
  ui::icon::{Icon, glyph},
};

/// The icon tile (`Icon tile` `cga8N`, `NNcZR`).
const TILE_SIZE: f32 = 40.0;

/// The tile above the heading: the app window for a screen about another window, the alert for a
/// failure.
pub fn tile(which: Icon, failure: bool) -> Row {
  let (fill, stroke, color) = if failure {
    (PaletteColor::DangerMuted, PaletteColor::Danger, PaletteColor::Danger)
  } else {
    (
      PaletteColor::SurfaceRaised,
      PaletteColor::Border,
      PaletteColor::TextSecondary,
    )
  };
  Row::new()
    .width(Dimension::Px(TILE_SIZE))
    .height(Dimension::Px(TILE_SIZE))
    .rounded(RadiusSize::Lg)
    .background(fill)
    .border_inside(BorderSize::Sm, stroke)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(
      Text::new(&glyph(which))
        .variant(StartupText::TileIcon.role())
        .color(color),
    )
}

/// The heading, what failed under it when the screen says so, and what that means.
pub fn heading(title: &str, reason: Option<&str>, description: &str) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(Text::new(title).variant(TypographyStyle::Heading))
    .with_children(reason.map(|reason| wrapping(reason, ExtraTypography::Strong.role())))
    .child(wrapping(description, TypographyStyle::Description))
}

/// What to do now, under its heading (`To do` `q1MJli`).
pub fn step(title: &str, body: &str) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(Text::new(title).variant(ExtraTypography::Strong.role()))
    .child(wrapping(body, TypographyStyle::Description))
}

/// The buttons in a row (`Actions` `PTgQA`).
pub fn actions(buttons: Vec<Element>) -> Row {
  Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(buttons)
}

/// A line under the actions; the log's path in the monospace face.
pub fn footnote(text: &str, path: bool) -> Text {
  let role = if path {
    ExtraTypography::MonoNote.role()
  } else {
    TypographyStyle::Caption
  };
  wrapping(text, role).selectable(path)
}

/// Text that wraps within its parent's width.
fn wrapping(text: &str, role: TypographyStyle) -> Text {
  Text::new(text).variant(role).width(Dimension::Pct(100.0))
}

/// Opens `path` with its default program, off the UI thread: the launcher can take a moment.
pub fn open(path: PathBuf) {
  let opening = thread::Builder::new().name("open-path".to_owned()).spawn(move || {
    if let Err(error) = external::open_path(&path) {
      tracing::error!(%error, path = %path.display(), "cannot open a path from a startup screen");
    }
  });
  if let Err(error) = opening {
    tracing::error!(%error, "cannot start opening a path from a startup screen");
  }
}
