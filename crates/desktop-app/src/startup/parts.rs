//! The parts every startup screen is made of (`g0Kj1`, `kMM56`): the icon tile, the heading with what
//! failed, keys and values, a path in its box with Copy, what to do, the actions and a footnote. The UI
//! kit's buttons and code box are not in this app yet; these follow the same design masters with the
//! theme's roles.

use std::{path::PathBuf, thread};

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  components::{Button, Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_exec::external;

use crate::{
  theme::{ExtraColor, ExtraSpacing, IconSize, StartupSpacing, StartupText},
  ui::icon::{Icon, glyph, icon},
};

/// The icon tile (`Icon tile` `cga8N`, `NNcZR`).
const TILE_SIZE: f32 = 40.0;
/// A button (`Switch to that window` `vNQTN`, `Open log` `kXPYl`).
const BUTTON_HEIGHT: f32 = 28.0;
/// Copy inside a path's box (`Copy` `YQUbn`).
const COPY_HEIGHT: f32 = 24.0;
/// A key beside its value (`Key` `rtxjz`).
const KEY_WIDTH: f32 = 108.0;

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
    .with_children(reason.map(|reason| wrapping(reason, StartupText::Reason.role())))
    .child(wrapping(description, TypographyStyle::Description))
}

/// A key and its value (`Detail · Data folder` `E197l`); a path in the monospace face.
pub fn fact(key: &str, value: &str, path: bool) -> Row {
  let value = if path {
    wrapping(value, StartupText::Path.role()).selectable(true)
  } else {
    wrapping(value, TypographyStyle::Description).color(PaletteColor::TextPrimary)
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(StartupSpacing::Fact.role()))
    .spacing(SpacingSize::Md)
    .child(
      Text::new(key)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Px(KEY_WIDTH)),
    )
    .child(value.flex(1.0).min_width(Dimension::Px(0.0)))
}

/// A path under its label, in its box with Copy (`Path` `FGy4r`).
pub fn path_box(label: &str, path: &str, copy: &str) -> Column {
  let copied = path.to_owned();
  let copy = quiet_button("startup-copy-path", Icon::Copy, copy, move || {
    if !lurq::clipboard::copy_to_clipboard(&copied) {
      tracing::warn!("cannot copy the path to the clipboard");
    }
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Text::new(label)
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary),
    )
    .child(
      Row::new()
        .id("startup-path")
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::vertical(ExtraSpacing::Snug.role())
            .left(ExtraSpacing::Loose.role())
            .right(SpacingSize::Xs),
        )
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .rounded(RadiusSize::Md)
        .background(ExtraColor::CodeBackground.role())
        .border_inside(BorderSize::Sm, PaletteColor::Border)
        .child(
          wrapping(path, StartupText::Path.role())
            .selectable(true)
            .flex(1.0)
            .min_width(Dimension::Px(0.0)),
        )
        .child(copy),
    )
}

/// What to do now, under its heading (`To do` `q1MJli`).
pub fn step(title: &str, body: &str) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(Text::new(title).variant(StartupText::Step.role()))
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
    StartupText::Log.role()
  } else {
    TypographyStyle::Caption
  };
  wrapping(text, role).selectable(path)
}

/// How a button looks.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Look {
  /// The one action of the screen: `accent`, its content `on_accent`.
  Primary,
  /// `surface_raised` inside a `border_strong` stroke, its content `text_primary`.
  Secondary,
}

/// A button 28 high: an optional 14 icon and its label, 6 apart and 12 from its sides.
pub fn button(
  id: &'static str,
  glyph: Option<Icon>,
  label: &str,
  look: Look,
  on_click: impl Fn() + Send + Sync + 'static,
) -> Element {
  let (content, button) = match look {
    Look::Primary => (
      ExtraColor::OnAccent.role(),
      pressed(id, label, on_click).background(PaletteColor::Accent),
    ),
    Look::Secondary => (
      PaletteColor::TextPrimary,
      pressed(id, label, on_click)
        .background(PaletteColor::SurfaceRaised)
        .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
        .hovered_style(Style::new().background(ExtraColor::Hover.role())),
    ),
  };
  button
    .height(Dimension::Px(BUTTON_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .with_children(glyph.map(|glyph| icon(glyph, IconSize::Md, content.clone())))
    .child(
      Text::new(label)
        .variant(TypographyStyle::Button)
        .color(content)
        .nowrap(),
    )
    .into()
}

/// Copy in a path's box: no fill until the pointer is over it, its content `text_secondary`.
fn quiet_button(id: &'static str, glyph: Icon, label: &str, on_click: impl Fn() + Send + Sync + 'static) -> Element {
  pressed(id, label, on_click)
    .height(Dimension::Px(COPY_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(label)
        .variant(TypographyStyle::Button)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .into()
}

/// A focusable button named `label` for assistive tools and agents.
fn pressed(id: &'static str, label: &str, on_click: impl Fn() + Send + Sync + 'static) -> Button {
  Button::empty()
    .id(id)
    .describe("a11y_name", label.to_owned())
    .cursor(CursorIcon::Pointer)
    .rounded(RadiusSize::Md)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .on_click(move |_| on_click())
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
