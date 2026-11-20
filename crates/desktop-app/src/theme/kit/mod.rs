//! The recipes of `lurq::ui`'s components that the kit (`ui/`) draws, patched to the design's values: each kit
//! primitive that renders a lurq component keeps its look here.

pub mod avatar;
pub mod callout;
pub mod chip;
mod kbd;
pub mod meter;

use lurq::app::theme::Theme;

pub(super) fn install(theme: &Theme) {
  avatar::install(theme);
  callout::install(theme);
  chip::install(theme);
  kbd::install(theme);
  meter::install(theme);
}
