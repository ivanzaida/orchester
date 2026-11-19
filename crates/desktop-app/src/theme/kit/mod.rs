//! The recipes of `lurq::ui`'s components that the kit (`ui/`) draws, patched to the design's values: each kit
//! primitive that renders a lurq component keeps its look here.

mod kbd;

use lurq::app::theme::Theme;

pub(super) fn install(theme: &Theme) {
  kbd::install(theme);
}
