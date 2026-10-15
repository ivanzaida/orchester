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
  node::{Element, HitTestBehavior, dimension::Dimension, padding::Padding},
};

use super::{
  app_menu::AppMenu,
  tab_strip::{PillRef, TabStrip, TabStripProps},
};
use crate::{shell::needs_you::NeedsYouPill, theme::ExtraSpacing};

/// The line between the app menu and the tabs (`Divider` `pE6GJ`).
const DIVIDER_HEIGHT: f32 = 16.0;

/// `bar` with the app menu and, when `tabs`, the divider and the tabs, and `lights` (the macOS window
/// controls) at its end; `closing` while the closing panel holds the window, when the menu and the pill
/// do not react either.
pub fn content(
  ctx: &mut Ctx,
  bar: ChromeTitleBar,
  (tabs, closing): (bool, bool),
  lights: Option<Element>,
) -> ChromeTitleBar {
  let inert = if closing {
    HitTestBehavior::None
  } else {
    HitTestBehavior::Auto
  };
  let menu = Row::new()
    .padding(Padding::new().left(ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(
      Row::new()
        .hit_test(inert)
        .child(ctx.mount_keyed::<AppMenu>("app-menu", ())),
    );
  if !tabs {
    let bar = bar.leading(menu);
    return match lights {
      Some(lights) => bar.trailing(lights),
      None => bar,
    };
  }
  let line = ctx.theme().border_size_value(BorderSize::Sm);
  // The tabs leave the pill the room it took when it was laid out last.
  let pill = ctx.element_ref();
  // The bar's 4 between the pill's room and the window controls (`DL8Xp` in `VnLOT`): the macOS lights are
  // ours, the Windows controls the bar's own, after its trailing part.
  let trailing = match lights {
    Some(_) => Row::new().spacing(SpacingSize::Xs),
    None => Row::new().padding(Padding::new().right(SpacingSize::Xs)),
  };
  bar
    .leading(
      menu.child(
        Row::new()
          .width(Dimension::Px(line))
          .height(Dimension::Px(DIVIDER_HEIGHT))
          .background(PaletteColor::Border),
      ),
    )
    .center(ctx.mount_keyed::<TabStrip>(
      "tab-strip",
      TabStripProps {
        pill: PillRef(pill.clone()),
        closing,
      },
    ))
    .trailing(
      trailing
        .align_items(Alignment::Center)
        .child(
          Row::new()
            .hit_test(inert)
            .child(ctx.mount_keyed::<NeedsYouPill>("needs-you", PillRef(pill))),
        )
        .with_children(lights),
    )
}
