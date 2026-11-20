//! What the title bar shows besides the window controls (`Title bar/Windows` `Wl7Kt`): at its start the app menu and
//! a divider, then the tabs; the room after them drags the window; at its end the *N need you* pill
//! (`shell::needs_you`). A startup screen's title bar has the app menu only (`WVRwk`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::Row,
  layout::Alignment,
  node::{Element, HitTestBehavior, dimension::Dimension, padding::Padding},
};

use super::{
  app_menu::AppMenu,
  tab_strip::{PillRef, TabStrip, TabStripProps},
};
use crate::{shell::needs_you::NeedsYouPill, theme::ExtraSpacing};

/// The line between the app menu and the tabs (`Divider` `dVhLf`).
const DIVIDER_HEIGHT: f32 = 16.0;

/// What the title bar holds besides the window controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar {
  /// A started session's: the app menu, the tabs and the pill.
  Tabs,
  /// A startup screen's: the app menu.
  Menu,
}

/// The bar's leading part and its trailing one for what `kind` holds; `closing` while the closing panel holds the
/// window, when the menu and the pill do not react either.
pub fn content(ctx: &mut Ctx, (kind, closing): (Bar, bool)) -> (Element, Option<Element>) {
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
    return (menu.into(), None);
  }
  let line = ctx.theme().border_size_value(BorderSize::Sm);
  // The tabs leave the pill the room it took when it was laid out last.
  let pill = ctx.element_ref();
  let leading = Row::new()
    .align_items(Alignment::Center)
    .child(
      menu.child(
        Row::new()
          .width(Dimension::Px(line))
          .height(Dimension::Px(DIVIDER_HEIGHT))
          .background(PaletteColor::Border),
      ),
    )
    .child(ctx.mount_keyed::<TabStrip>(
      "tab-strip",
      TabStripProps {
        pill: PillRef(pill.clone()),
        closing,
      },
    ));
  let trailing = Row::new()
    .hit_test(inert)
    .child(ctx.mount_keyed::<NeedsYouPill>("needs-you", PillRef(pill)));
  (leading.into(), Some(trailing.into()))
}
