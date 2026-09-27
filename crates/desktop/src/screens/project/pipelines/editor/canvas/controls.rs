//! Over the canvas's bottom edge: the zoom controls (`iarCw`: zoom out, zoom in, the level —
//! which goes back to 100 % — and fit) and, right after publishing, the published notice
//! (`O8ivsy`). The row itself lets presses and the wheel through to the canvas.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, HitTestBehavior, Style, border::Border, dimension::Dimension},
};
use orchester_core::resource::VersionNumber;

use super::input::ZoomCommand;
use crate::{
  pipelines::zoom::Zoom,
  theme::ExtraSpacing,
  ui::{
    icon::{Icon, IconSize, icon},
    notice::Notice,
    pressable::{pressable, surface},
  },
};

/// Zoom controls (`iarCw`) and the published notice (`O8ivsy`); each zoom button's square,
/// centered on its icon.
const ZOOM_WIDTH: f32 = 152.0;
const ZOOM_HEIGHT: f32 = 32.0;
const ZOOM_BUTTON: f32 = 20.0;
const NOTICE_WIDTH: f32 = 396.0;
/// Room the controls take at the bottom of the canvas: their height and the padding under them.
pub const CONTROLS_ROOM: f32 = 56.0;

type Command = Arc<dyn Fn(ZoomCommand) + Send + Sync>;

pub fn overlay(ctx: &Ctx, zoom: Zoom, command: &Command, published: Option<VersionNumber>) -> Element {
  let mut bar = Row::new()
    .width(Dimension::Pct(100.0))
    .hit_test(HitTestBehavior::ContentOnly)
    .padding_horizontal(ExtraSpacing::Indent.role())
    .padding_bottom(SpacingSize::Lg)
    .spacing(SpacingSize::Md)
    .align_items(Alignment::End)
    .child(controls(ctx, zoom, command))
    .child(Spacer::new().flex(1.0));
  if let Some(version) = published {
    let version = version.to_string();
    bar = bar.child(
      Column::new().width(Dimension::Px(NOTICE_WIDTH)).child(
        Notice::new(
          Icon::CircleCheck,
          ctx.t_args("pipelines.published.title", [("version", version.as_str())]),
          ctx.t_args("pipelines.published.detail", [("version", version.as_str())]),
        )
        .id("pipeline-published-notice"),
      ),
    );
  }
  bar.into()
}

fn controls(ctx: &Ctx, zoom: Zoom, command: &Command) -> Element {
  let level = Text::new(&ctx.t_args(
    "pipelines.canvas.zoom",
    [("percent", zoom.percent().to_string().as_str())],
  ))
  .variant(TypographyStyle::Mono)
  .color(PaletteColor::TextPrimary);
  let full = {
    let command = command.clone();
    pressable(
      surface()
        .id("pipeline-zoom-full")
        .align_items(Alignment::Center)
        .child(level),
      move || command(ZoomCommand::Full),
    )
  };
  Row::new()
    .id("pipeline-zoom")
    .describe("zoom", zoom.percent().to_string())
    .width(Dimension::Px(ZOOM_WIDTH))
    .height(Dimension::Px(ZOOM_HEIGHT))
    .padding_horizontal(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .justify(Justify::SpaceBetween)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Sm)
    .child(zoom_button(
      "pipeline-zoom-out",
      Icon::Minus,
      command,
      ZoomCommand::Out,
      zoom != Zoom::MIN,
    ))
    .child(zoom_button(
      "pipeline-zoom-in",
      Icon::Plus,
      command,
      ZoomCommand::In,
      zoom != Zoom::MAX,
    ))
    .child(full)
    .child(zoom_button(
      "pipeline-zoom-fit",
      Icon::Maximize,
      command,
      ZoomCommand::Fit,
      true,
    ))
    .into()
}

/// A zoom control: its icon on a square that fills on hover; at the end of the range it does
/// nothing and says so with a fainter icon.
fn zoom_button(id: &'static str, glyph: Icon, command: &Command, action: ZoomCommand, available: bool) -> Button {
  let color = if available {
    PaletteColor::TextSecondary
  } else {
    PaletteColor::TextMuted
  };
  let button = surface()
    .id(id)
    .size(Dimension::Px(ZOOM_BUTTON), Dimension::Px(ZOOM_BUTTON))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(RadiusSize::Sm)
    .child(icon(glyph, IconSize::Marker, color));
  if !available {
    return button.describe("state", "disabled").cursor(CursorIcon::NotAllowed);
  }
  let command = command.clone();
  pressable(button, move || command(action)).hovered_style(Style::new().background(PaletteColor::AccentMuted))
}
