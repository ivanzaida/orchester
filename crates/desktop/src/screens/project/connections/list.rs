//! A connection in the project list (`sFtAz` as in `d9bo3`): icon, name and what it runs, its
//! type, how many agents use it, its status and a chevron. The selected row is raised.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::connection::Connection;

use super::status::{CHECKING, program_key, status_label, vendor_key};
use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    status_label::status_label as label,
  },
};

/// Row, icon surface and fixed column sizes (`d9bo3`, `lhuOK`, `JIsjZ`, `f6poW`).
const ROW_HEIGHT: f32 = 88.0;
const ICON_SURFACE: f32 = 36.0;
const TYPE_WIDTH: f32 = 76.0;
const USAGE_WIDTH: f32 = 58.0;

pub struct RowState {
  pub uses: usize,
  pub selected: bool,
  pub checking: bool,
}

pub fn connection_row(
  ctx: &Ctx,
  connection: &Connection,
  state: &RowState,
  on_click: impl Fn() + Send + Sync + 'static,
) -> Element {
  let provider = connection.provider();
  let (vendor, program) = (ctx.t(vendor_key(provider)), ctx.t(program_key(provider)));
  let description = match connection.version() {
    Some(version) => ctx.t_args(
      "connections.row.description",
      [("vendor", &*vendor), ("program", &*program), ("version", version)],
    ),
    None => ctx.t_args(
      "connections.row.descriptionUnchecked",
      [("vendor", &*vendor), ("program", &*program)],
    ),
  };
  let usage_key = if state.uses == 1 {
    "connections.usage.one"
  } else {
    "connections.usage.other"
  };
  let (status_icon, status_key) = if state.checking {
    CHECKING
  } else {
    status_label(connection.status())
  };
  let identity = Column::new()
    .flex(1.0)
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(connection.name().as_str())
        .variant(TypographyStyle::Label)
        .nowrap(),
    )
    .child(
      Text::new(&description)
        .variant(ExtraTypography::Footnote.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    );
  let row = Row::new()
    .id(Arc::<str>::from(format!("connection-{}", connection.id())))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .background(if state.selected {
      PaletteColor::SurfaceRaised
    } else {
      ExtraColor::Surface.role()
    })
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(icon_surface())
    .child(identity)
    .child(fixed_caption(&ctx.t("connections.type.provider"), TYPE_WIDTH))
    .child(fixed_caption(
      &ctx.t_args(usage_key, [("count", state.uses.to_string().as_str())]),
      USAGE_WIDTH,
    ))
    .child(label(status_icon, &ctx.t(status_key)))
    .child(icon(Icon::ChevronRight, IconSize::Dismiss, PaletteColor::TextSecondary));
  if state.selected {
    return row.describe("state", "selected").into();
  }
  row
    .cursor(CursorIcon::Pointer)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .on_click(move |_| on_click())
    .into()
}

fn icon_surface() -> Element {
  Row::new()
    .size(Dimension::Px(ICON_SURFACE), Dimension::Px(ICON_SURFACE))
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(Icon::Bot, IconSize::Notice, PaletteColor::TextPrimary))
    .into()
}

fn fixed_caption(text: &str, width: f32) -> Element {
  Column::new()
    .width(Dimension::Px(width))
    .child(Text::new(text).variant(TypographyStyle::Caption).nowrap())
    .into()
}
