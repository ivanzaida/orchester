//! The shell around every screen (`Body` and `Status bar` of `Overview · full page · dark` `q9mf0k` and
//! `Tasks · dark` `jn2IT`): the sidebar, the rounded inset content panel that hosts the routed screen,
//! and the status bar. All projects and Add project have no sidebar: their panel is inset 8 on both sides
//! (`Content wrap` `cV43B` in `S5HDKf`, `jJ0BM` in `kkGWj`). The title bar belongs to the window chrome
//! (`crate::chrome`).

mod clock;
pub mod needs_you;
pub mod sidebar;
pub mod status_bar;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Outlet, Row},
  node::{Element, dimension::Dimension, padding::Padding},
};

use self::{sidebar::Sidebar, status_bar::StatusBar};
use crate::{
  routes::{Route, Screen},
  theme::ExtraColor,
};

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
  // A path that names no route keeps the sidebar; its screen says why it names none (`screens::not_found`).
  let screen = Route::parse(&ctx.route_path()).ok().map(|route| route.screen());
  let sidebar = !matches!(screen, Some(Screen::AllProjects | Screen::AddProject));
  // `Content wrap` (`dHxPJ`): 8 between the panel and the window's right edge, and its left without a sidebar.
  let wrap = Padding::new().right(SpacingSize::Sm);
  let wrap = if sidebar { wrap } else { wrap.left(SpacingSize::Sm) };
  let body = Row::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .with_children(sidebar.then(|| ctx.mount_keyed::<Sidebar>("sidebar", ())))
    .child(
      Column::new()
        .height(Dimension::Pct(100.0))
        .flex(1.0)
        .padding(wrap)
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
