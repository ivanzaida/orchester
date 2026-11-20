//! Menus (`ui::menu`): lurq's `ui::Menu` and `ui::MenuItem` in the design's look. The panel is `surface_raised` inside
//! a `border_strong` stroke, radius 8, 4 inside and 2 between rows, lifted by the `md` shadow (`Pipeline menu` `ClKqz`,
//! `Tag menu` `LLIjG`); a filter's menu is radius 10, 6 inside, lifted by the `lg` shadow (`CLI menu` `tEymU`), as the
//! rules [`filter_panel`] lay over it. Groups are split by a 1 `border` line with 4 over and under it (`Menu separator`
//! `UoDdI`). An item is the kit's own row (`MenuItem::content`): the item's root is at least 28 high, radius 4, with no
//! padding of its own, `hover` under the pointer, under the keyboard and while it is the active one, and faded while it
//! cannot be chosen. Both recipes are the app's whole: lurq's 230 to 360 wide panel and its 32 high rows are not the
//! design's.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize, Theme},
  node::{CursorIcon, border::Border},
  ui::{Menu, MenuItem, MenuItemPart, MenuItemState, MenuPart, Recipe, Selector},
};

use crate::theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraRadius, ExtraSpacing};

/// An item that is a task form chip's option (`screens::task_form::chips::MenuOption`, `I2BrOT`, `yufU5`): radius 6,
/// and the `hover` fill while it is the choice.
pub const OPTION: &str = "option";

pub(super) fn install(theme: &Theme) {
  theme.set_recipe::<Menu>(
    Recipe::new()
      .base(MenuPart::Panel, |style| {
        style
          .padding_x(SpacingSize::Xs)
          .padding_y(SpacingSize::Xs)
          .gap(ExtraSpacing::Tight.role())
          .radius(ExtraRadius::Tile.role())
          .background(PaletteColor::Card)
          .border(Border::inside(BorderSize::Sm, PaletteColor::Input))
          .shadow(ShadowStyle::Md)
      })
      .base(MenuPart::Separator, |style| style.padding_y(SpacingSize::Xs))
      .base(MenuPart::SeparatorLine, |style| {
        style.height(1.0).background(PaletteColor::Border)
      }),
  );
  let fill = |style: lurq::ui::PartStyle| style.background(ExtraColor::Hover.role());
  theme.set_recipe::<MenuItem>(
    Recipe::new()
      .base(MenuItemPart::Root, |style| {
        style
          .min_height(ControlHeight::Md.value())
          .radius(RadiusSize::Sm)
          .cursor(CursorIcon::Pointer)
      })
      .hovered(MenuItemPart::Root, fill)
      .focus_visible(MenuItemPart::Root, fill)
      .state(MenuItemState::Active, MenuItemPart::Root, fill)
      .state(OPTION, MenuItemPart::Root, |style| style.radius(RadiusSize::Md))
      .when(
        Selector::new().state(OPTION).state(MenuItemState::Checked),
        MenuItemPart::Root,
        fill,
      )
      .state(MenuItemState::Disabled, MenuItemPart::Root, |style| {
        style.opacity(DISABLED_OPACITY).cursor(CursorIcon::Default)
      }),
  );
}

/// A filter's menu (`CLI menu` `tEymU`): radius 10, 6 inside, lifted by the `lg` shadow.
pub fn filter_panel() -> Recipe {
  Recipe::new().base(MenuPart::Panel, |style| {
    style
      .padding_x(ExtraSpacing::Snug.role())
      .padding_y(ExtraSpacing::Snug.role())
      .radius(RadiusSize::Lg)
      .shadow(ShadowStyle::Lg)
  })
}
