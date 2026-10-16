//! What the title bar shows besides the window controls (`Title bar` `yZzwy`): the app menu and a
//! divider, then the tabs in the bar's flexible middle, which ends in drag area, and the *N need you* pill
//! before the window controls (`shell::needs_you`). A startup screen's title bar has the app menu only
//! (`bcDe3`); the sign-in gate's has the mark only, which opens nothing (`Brand` `psPqB` in `dd04F`).

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
use crate::{brand::mark, shell::needs_you::NeedsYouPill, theme::ExtraSpacing};

/// The line between the app menu and the tabs (`Divider` `pE6GJ`).
const DIVIDER_HEIGHT: f32 = 16.0;
/// The mark's room on the sign-in gate, as high as the app menu's button (`Brand` `psPqB`).
const BRAND_HEIGHT: f32 = 28.0;

/// What the title bar holds besides the window controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar {
  /// A started session's: the app menu, the tabs and the pill.
  Tabs,
  /// A startup screen's: the app menu.
  Menu,
  /// The sign-in gate's: the mark.
  Mark,
}

/// `bar` with what `kind` holds, and `lights` (the macOS window controls) at its end; `closing` while the closing
/// panel holds the window, when the menu and the pill do not react either.
pub fn content(
  ctx: &mut Ctx,
  bar: ChromeTitleBar,
  (kind, closing): (Bar, bool),
  lights: Option<Element>,
) -> ChromeTitleBar {
  let ends = |bar: ChromeTitleBar, leading: Row| {
    let bar = bar.leading(leading);
    match lights.clone() {
      Some(lights) => bar.trailing(lights),
      None => bar,
    }
  };
  if kind == Bar::Mark {
    let brand = Row::new()
      .id("title-bar-brand")
      .height(Dimension::Px(BRAND_HEIGHT))
      .padding(Padding::horizontal(ExtraSpacing::Snug.role()).left(SpacingSize::Xs))
      .align_items(Alignment::Center)
      .child(mark());
    return ends(
      bar,
      Row::new()
        .padding(Padding::new().left(ExtraSpacing::Snug.role()))
        .align_items(Alignment::Center)
        .child(brand),
    );
  }
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
  if kind == Bar::Menu {
    return ends(bar, menu);
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
