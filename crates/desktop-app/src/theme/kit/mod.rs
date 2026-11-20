//! The recipes of `lurq::ui`'s components that the kit (`ui/`) draws, patched to the design's values: each kit
//! primitive that renders a lurq component keeps its look here.

mod button;
mod icon_button;
mod kbd;
mod segmented;
mod trigger;

use lurq::app::theme::Theme;

pub(super) fn install(theme: &Theme) {
  button::install(theme);
  icon_button::install(theme);
  kbd::install(theme);
  segmented::install(theme);
  trigger::install(theme);
}
