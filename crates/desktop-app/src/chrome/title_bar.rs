//! What the title bar shows besides the window controls (`Title bar` `yZzwy`): the app menu, a divider
//! and the tabs. The design's trailing *Needs you* count comes with its data; until then the rest of the
//! bar is the drag area.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
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
  bar.leading(
    Row::new()
      .padding(Padding::new().left(ExtraSpacing::Snug.role()))
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .child(ctx.mount_keyed::<AppMenu>("app-menu", ()))
      .child(
        Row::new()
          .width(Dimension::Px(1.0))
          .height(Dimension::Px(DIVIDER_HEIGHT))
          .background(PaletteColor::Border),
      )
      .child(ctx.mount_keyed::<TabStrip>("tab-strip", ())),
  )
}
