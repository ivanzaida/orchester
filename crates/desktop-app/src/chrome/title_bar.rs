//! What the title bar shows besides the window controls (`Title bar` `yZzwy`): the app menu and a
//! divider, then the tabs in the bar's flexible middle, which ends in drag area, and the *N need you* pill
//! before the window controls (`shell::needs_you`). A startup screen's title bar has the app menu only
//! (`bcDe3`).

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
use crate::{shell::needs_you::NeedsYouPill, theme::ExtraSpacing};

/// The line between the app menu and the tabs (`Divider` `pE6GJ`).
const DIVIDER_HEIGHT: f32 = 16.0;

/// `bar` with the app menu and, when `tabs`, the divider and the tabs.
pub fn content(ctx: &mut Ctx, bar: ChromeTitleBar, tabs: bool) -> ChromeTitleBar {
  let menu = Row::new()
    .padding(Padding::new().left(ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(ctx.mount_keyed::<AppMenu>("app-menu", ()));
  if !tabs {
    return bar.leading(menu);
  }
  let line = ctx.theme().border_size_value(BorderSize::Sm);
  bar
    .leading(
      menu.child(
        Row::new()
          .width(Dimension::Px(line))
          .height(Dimension::Px(DIVIDER_HEIGHT))
          .background(PaletteColor::Border),
      ),
    )
    .center(ctx.mount_keyed::<TabStrip>("tab-strip", ()))
    .trailing(ctx.mount_keyed::<NeedsYouPill>("needs-you", ()))
}
