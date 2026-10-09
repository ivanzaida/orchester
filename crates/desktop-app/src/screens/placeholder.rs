//! What a screen that isn't built yet shows: its icon, its name and that it isn't built, in the content
//! panel's middle, under the screen's own element id `screen-<name>` so tests and agents find it.
//! Nothing is made up.

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
  routes::Screen,
  theme::IconSize,
  ui::icon::{Icon, icon, screen_icon},
};

/// `screen`'s placeholder under the screen's name.
pub fn placeholder(ctx: &mut Ctx, screen: Screen) -> Element {
  let title = ctx.t(screen.title_key());
  titled(ctx, screen, &title)
}

/// `screen`'s placeholder under `title`, for a screen that names what it shows (a task's key).
pub fn titled(ctx: &mut Ctx, screen: Screen, title: &str) -> Element {
  let body = ctx.t("shell.placeholder.notBuilt");
  centered(screen_icon(screen), title, &body)
    .id(format!("screen-{}", screen.name()))
    .into()
}

/// An icon, a title and a line under it, in the middle of the content panel.
pub fn centered(glyph: Icon, title: &str, body: &str) -> Column {
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
