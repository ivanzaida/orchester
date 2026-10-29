//! An opened item's top (`Crumb` and `Page head` in 03b, 04b): the crumb back to its tab's list, then its tile, name
//! and what it is, with the … menu holding Remove at the right.
use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::parts::tile;
use crate::{
  navigation::link::Open,
  theme::{ExtraSpacing, HubText, IconSize},
  ui::{
    icon::{Icon, icon},
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
    pressable::opening,
  },
};

/// ← `list`, back to the tab's list, then `/` and `here` on a page of its own (`Crumb` `pPjI1` in 03c, `bnei1` in 02c).
pub(crate) fn crumb(ctx: &Ctx, id: &str, (list, back): (&str, Open), here: Option<&str>) -> Element {
  let back = opening(back)
    .id(id.to_owned())
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::ArrowLeft, IconSize::Base, PaletteColor::TextMuted))
    .child(Text::new(list).variant(HubText::Crumb.role()));
  let here = here.map(|here| {
    [
      Text::new(&ctx.t("hub.crumbSeparator"))
        .id(format!("{id}-separator"))
        .variant(HubText::Crumb.role()),
      Text::new(here)
        .id(format!("{id}-here"))
        .variant(HubText::Crumb.role())
        .color(PaletteColor::TextSecondary),
    ]
  });
  Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(back)
    .with_children(here.into_iter().flatten())
    .into()
}

/// The … menu's width, as wide as the app menu's: without one its panel spans the window.
pub(crate) const MORE_MENU_WIDTH: f32 = 208.0;

/// The … menu: Remove.
pub(crate) struct More {
  pub open: Signal<bool>,
  pub anchor: ElementRef,
  pub remove: Arc<dyn Fn() + Send + Sync>,
}

/// The head of the item `id` names (`<id>-page-head`, `<id>-title`, `<id>-sub`, `<id>-more`, `<id>-remove`).
pub(crate) fn head(ctx: &mut Ctx, id: &str, (glyph, title, sub): (Icon, &str, &str), more: Option<More>) -> Element {
  let menu = more.map(|more| {
    let remove = more.remove.clone();
    let item = MenuItem::new(format!("{id}-remove"), ctx.t("hub.remove.action"), move || remove())
      .icon(Icon::Trash2)
      .render(ctx, &more.open);
    let open = more.open.clone();
    let button = IconButton::new(format!("{id}-more"), Icon::Ellipsis, ctx.t("hub.more"))
      .small()
      .element(more.anchor.clone())
      .on_click(move || open.update(|open| *open = !*open))
      .render(ctx);
    let panel = menu_panel(&format!("{id}-more-menu"))
      .width(Dimension::Px(MORE_MENU_WIDTH))
      .child(item);
    Column::new()
      .child(button)
      .child(menu_at_end(more.anchor, more.open, panel))
  });
  Row::new()
    .id(format!("{id}-page-head"))
    .width(Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(tile(glyph, true))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(
          Text::new(title)
            .id(format!("{id}-title"))
            .variant(HubText::PageTitle.role()),
        )
        .child(Text::new(sub).id(format!("{id}-sub")).variant(HubText::Hint.role())),
    )
    .with_children(menu)
    .into()
}
