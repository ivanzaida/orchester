//! What the title bar shows besides the window controls (`Title bar` `yZzwy`): the app menu and a
//! divider, then the tabs in the bar's flexible middle, which ends in drag area. The design's trailing
//! *Needs you* count comes with its data.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{ChromeTitleBar, Row},
  layout::Alignment,
  node::{dimension::Dimension, padding::Padding},
};

use super::{app_menu::AppMenu, tab_strip::TabStrip};
use crate::theme::ExtraSpacing;

/// The line between the app menu and the tabs (`Divider` `pE6GJ`).
const DIVIDER_HEIGHT: f32 = 16.0;

pub fn content(ctx: &mut Ctx, bar: ChromeTitleBar) -> ChromeTitleBar {
  let line = ctx.theme().border_size_value(BorderSize::Sm);
  bar
    .leading(
      Row::new()
        .padding(Padding::new().left(ExtraSpacing::Snug.role()))
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::Center)
        .child(ctx.mount_keyed::<AppMenu>("app-menu", ()))
        .child(
          Row::new()
            .width(Dimension::Px(line))
            .height(Dimension::Px(DIVIDER_HEIGHT))
            .background(PaletteColor::Border),
        ),
    )
    .center(ctx.mount_keyed::<TabStrip>("tab-strip", ()))
}
