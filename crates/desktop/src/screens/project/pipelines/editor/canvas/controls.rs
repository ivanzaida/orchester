//! The band along the canvas's bottom edge: the zoom controls (`iarCw`: zoom out, zoom in, the
//! level — which goes back to 100 % — and fit) and, right after publishing, the published
//! notice (`O8ivsy`). The notice is taller than the band, so it floats over the canvas, anchored
//! to the band's right end with its bottom level with the controls'.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::{Ctx, Placement},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Popup, Row, Spacer, Text},
  core::ElementRef,
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, HitTestBehavior, Style, border::Border, dimension::Dimension},
};
use orchester_core::resource::VersionNumber;

use super::input::ZoomCommand;
use crate::{
  pipelines::zoom::Zoom,
  theme::{ExtraSpacing, UNAVAILABLE_OPTION_OPACITY},
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
/// Height of the band: the controls and the padding under them.
const BAND_HEIGHT: f32 = 56.0;

type Command = Arc<dyn Fn(ZoomCommand) + Send + Sync>;

/// The band; `anchor` marks where the published notice ends.
pub fn band(
  ctx: &Ctx,
  zoom: Zoom,
  command: &Command,
  published: Option<VersionNumber>,
  anchor: &ElementRef,
) -> Element {
  let bar = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(BAND_HEIGHT))
    .padding_left(ExtraSpacing::Indent.role())
    .padding_right(SpacingSize::Lg)
    .padding_bottom(SpacingSize::Lg)
    .spacing(SpacingSize::Md)
    .align_items(Alignment::End)
    .child(controls(ctx, zoom, command))
    .child(Spacer::new().flex(1.0))
    .child(
      Column::new()
        .width(Dimension::Px(NOTICE_WIDTH))
        .height(Dimension::Px(ZOOM_HEIGHT))
        .ref_element(anchor.clone()),
    );
  let Some(version) = published else {
    return bar.into();
  };
  let version = version.to_string();
  let notice = Column::new().width(Dimension::Px(NOTICE_WIDTH)).child(
    Notice::new(
      Icon::CircleCheck,
      ctx.t_args("pipelines.published.title", [("version", version.as_str())]),
      ctx.t_args("pipelines.published.detail", [("version", version.as_str())]),
    )
    .id("pipeline-published-notice"),
  );
  let floating = Popup::new(anchor.clone(), notice)
    .open_when(true)
    .placement(Placement::TopEnd)
    // Above the anchor with a negative gap of the anchor's height: the notice ends where it does.
    .offset(0.0, -ZOOM_HEIGHT)
    .hit_test(HitTestBehavior::None)
    .dismiss_on_outside_click(false)
    .dismiss_on_escape(false);
  bar.child(floating).into()
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
        .height(Dimension::Px(ZOOM_BUTTON))
        .align_items(Alignment::Center)
        .rounded(RadiusSize::Sm)
        .child(level),
      move || command(ZoomCommand::Full),
    )
    .hovered_style(hovered())
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
/// nothing and is dimmed like other controls that cannot be used.
fn zoom_button(id: &'static str, glyph: Icon, command: &Command, action: ZoomCommand, available: bool) -> Button {
  let button = surface()
    .id(id)
    .size(Dimension::Px(ZOOM_BUTTON), Dimension::Px(ZOOM_BUTTON))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(RadiusSize::Sm)
    .child(icon(glyph, IconSize::Marker, PaletteColor::TextSecondary));
  if !available {
    return button
      .describe("state", "disabled")
      .opacity(UNAVAILABLE_OPTION_OPACITY)
      .cursor(CursorIcon::NotAllowed);
  }
  let command = command.clone();
  pressable(button, move || command(action)).hovered_style(hovered())
}

fn hovered() -> Style {
  Style::new().background(PaletteColor::AccentMuted)
}
