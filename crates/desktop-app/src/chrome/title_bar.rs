//! What the title bar shows besides the window controls (`Title bar/Windows` `Wl7Kt`): at its start the app menu and
//! a divider, then the tabs; the room after them drags the window; at its end the *N need you* pill
//! (`shell::needs_you`). A startup screen's title bar has the app menu only (`WVRwk`). lurq stands a bar's parts on
//! its bottom edge where it has tabs: the menu and the pill are as high as the bar, their content in its middle.

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
use crate::{chrome::TITLE_BAR_HEIGHT, shell::needs_you::NeedsYouPill, theme::ExtraSpacing};

/// The line between the app menu and the tabs (`Divider` `dVhLf`).
const DIVIDER_HEIGHT: f32 = 16.0;

/// The bar's parts: before the tabs, the tabs, and after the room that drags the window.
pub struct Parts {
  pub leading: Element,
  pub tabs: Option<Element>,
  pub trailing: Option<Element>,
}

/// What the title bar holds besides the window controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar {
  /// A started session's: the app menu, the tabs and the pill.
  Tabs,
  /// A startup screen's: the app menu.
  Menu,
}

/// The bar's parts for what `kind` holds: its leading part, its tabs and its trailing part; `closing` while the
/// closing panel holds the window, when the menu and the pill do not react either.
pub fn content(ctx: &mut Ctx, (kind, closing): (Bar, bool)) -> Parts {
  let inert = if closing {
    HitTestBehavior::None
  } else {
    HitTestBehavior::Auto
  };
  let menu = Row::new()
    .height(Dimension::Px(TITLE_BAR_HEIGHT))
    .padding(Padding::new().left(ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(
      Row::new()
        .hit_test(inert)
        .child(ctx.mount_keyed::<AppMenu>("app-menu", ())),
    );
  if kind == Bar::Menu {
    return Parts {
      leading: menu.into(),
      tabs: None,
      trailing: None,
    };
  }
  let line = ctx.theme().border_size_value(BorderSize::Sm);
  // The tabs leave the pill the room it took when it was laid out last.
  let pill = ctx.element_ref();
  let leading = menu.child(
    Row::new()
      .width(Dimension::Px(line))
      .height(Dimension::Px(DIVIDER_HEIGHT))
      .background(PaletteColor::Border),
  );
  let tabs = ctx.mount_keyed::<TabStrip>(
    "tab-strip",
    TabStripProps {
      pill: PillRef(pill.clone()),
      closing,
    },
  );
  let trailing = Row::new()
    .height(Dimension::Px(TITLE_BAR_HEIGHT))
    .align_items(Alignment::Center)
    .hit_test(inert)
    .child(ctx.mount_keyed::<NeedsYouPill>("needs-you", PillRef(pill)));
  Parts {
    leading: leading.into(),
    tabs: Some(tabs),
    trailing: Some(trailing.into()),
  }
}
