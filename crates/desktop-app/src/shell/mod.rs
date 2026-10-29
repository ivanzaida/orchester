//! The shell around every screen (`Body` and `Status bar` of `Overview · full page · dark` `q9mf0k` and
//! `Tasks · dark` `jn2IT`): the sidebar, the rounded inset content panel that hosts the routed screen,
//! and the status bar. All projects and Add project have no sidebar: their panel is inset 8 on both sides
//! (`Content wrap` `cV43B` in `S5HDKf`, `jJ0BM` in `kkGWj`). *Jump to* opens over the content panel
//! (`jump`). The title bar belongs to the window chrome (`crate::chrome`).

pub(crate) mod clock;
pub mod jump;
pub mod needs_you;
pub mod sidebar;
pub mod status_bar;

use lurq::{
  app::{
    ctx::Ctx,
    events::MouseEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Outlet, Row},
  node::{Element, dimension::Dimension, padding::Padding},
};

use self::{jump::JumpTo, sidebar::Sidebar, status_bar::StatusBar};
use crate::{
  navigation::Navigation,
  routes::{Route, Screen},
  theme::ExtraColor,
};

pub fn layout(ctx: &mut Ctx) -> Element {
  let navigation = Navigation::of(ctx);
  // A press on the page closes *Jump to*, which opens over it, and goes on to the page.
  let jump = navigation.jump();
  // `Content · Overview` (`B2Mbdt`): `surface_panel`, radius `radius_lg`, a `border_panel` stroke inside.
  let content = Column::new()
    .id("content")
    .ref_element(navigation.jump_anchor())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .rounded(RadiusSize::Lg)
    .border_inside(BorderSize::Sm, ExtraColor::PanelBorder.role())
    .clip()
    .on_mouse_down(move |_: MouseEvent| {
      if jump.get_untracked() {
        jump.set(false);
      }
    })
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
        .child(ctx.mount_keyed::<JumpTo>("jump-to", ()))
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
