//! A status item's card on hover (`ui::hover_card`, `tKtTc`, `oagTL`): lurq's `ui::HoverCard`, whose panel and text are
//! the kit's own popover panel (`ui::popover`), so its recipe draws nothing of its own: no fill, edge, shadow, padding
//! or fade. Its distance from the item is the instance's offset. The recipe is the app's whole.

use lurq::{
  app::theme::Theme,
  ui::{HoverCard, Recipe},
};

pub(super) fn install(theme: &Theme) {
  theme.set_recipe::<HoverCard>(Recipe::new());
}
