//! Popovers (`ui::popover`): lurq's `ui::Popover`, whose panel is the kit's own popover panel (`ui::popover::
//! popover_panel`), so its recipe draws nothing of its own: no fill, edge, shadow, padding, gap or fade. Its distance
//! from its control is the instance's offset. The recipe is the app's whole.

use lurq::{
  app::theme::Theme,
  ui::{Popover, Recipe},
};

pub(super) fn install(theme: &Theme) {
  theme.set_recipe::<Popover>(Recipe::new());
}
