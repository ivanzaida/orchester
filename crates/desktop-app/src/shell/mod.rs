//! The shell around every screen (`Body` and `Status bar` of `Overview · full page · dark` `q9mf0k` and
//! `Tasks · dark` `jn2IT`): the sidebar, the rounded inset content panel that hosts the routed screen,
//! and the status bar. The title bar belongs to the window chrome (`crate::chrome`).

mod sidebar;
mod status_bar;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Outlet, Row},
  node::{Element, dimension::Dimension, padding::Padding},
};

use self::{sidebar::Sidebar, status_bar::StatusBar};
use crate::theme::ExtraColor;

pub fn layout(ctx: &mut Ctx) -> Element {
  // `Content · Overview` (`B2Mbdt`): `surface_panel`, radius `radius_lg`, a `border_panel` stroke inside.
  let content = Column::new()
    .id("content")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .rounded(RadiusSize::Lg)
    .border_inside(BorderSize::Sm, ExtraColor::PanelBorder.role())
    .clip()
    .child(Outlet::mount(ctx));
  let body = Row::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .child(ctx.mount_keyed::<Sidebar>("sidebar", ()))
    .child(
      // `Content wrap` (`dHxPJ`): 8 between the panel and the window's right edge.
      Column::new()
        .height(Dimension::Pct(100.0))
        .flex(1.0)
        .padding(Padding::new().right(SpacingSize::Sm))
        .child(content),
    );
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceBase)
    .child(body)
    .child(ctx.mount_keyed::<StatusBar>("status-bar", ()))
    .into()
}
