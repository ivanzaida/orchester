//! The screens the router shows inside the shell. None is built yet: every route shows an honest
//! placeholder that names its screen, and an address that names no screen says so. No content is made
//! up.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};

use crate::{
  routes::{ProjectPage, Route},
  theme::IconSize,
  ui::icon::{Icon, icon, screen_icon},
};

/// The screen at the router's path.
pub fn screen(ctx: &mut Ctx) -> Element {
  match Route::parse(&ctx.route_path()) {
    Some(route) => placeholder(ctx, &route),
    None => not_found(ctx),
  }
}

/// A path that names no screen, or names one with an identifier no route can hold.
pub fn not_found(ctx: &mut Ctx) -> Element {
  let path = ctx.route_path();
  let title = ctx.t("notFound.title");
  let body = ctx.t_args("notFound.body", [("path", path)]);
  centered(Icon::CircleHelp, &title, &body).id("screen-not-found").into()
}

fn placeholder(ctx: &mut Ctx, route: &Route) -> Element {
  let title = match route {
    Route::Project(_, ProjectPage::Task(key, tab)) => {
      let tab = ctx.t(tab.title_key());
      ctx.t_args("screens.taskTab", [("key", key.as_str()), ("tab", &tab)])
    }
    route => ctx.t(route.screen().title_key()),
  };
  let body = ctx.t("placeholder.notBuilt");
  centered(screen_icon(route.screen()), &title, &body)
    .id("screen-placeholder")
    .describe("screen", route.screen().name())
    .into()
}

fn centered(glyph: Icon, title: &str, body: &str) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .spacing(SpacingSize::Sm)
    .child(icon(glyph, IconSize::Lg, PaletteColor::TextMuted))
    .child(Text::new(title).variant(TypographyStyle::Title))
    .child(Text::new(body).variant(TypographyStyle::Description))
}
