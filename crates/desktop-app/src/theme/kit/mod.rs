//! The recipes of `lurq::ui`'s components that the kit (`ui/`) draws, patched to the design's values: each kit
//! primitive that renders a lurq component keeps its look here.

pub mod avatar;
mod button;
pub mod callout;
pub mod card;
pub mod chip;
pub mod detail_row;
mod icon_button;
mod kbd;
pub mod meter;
pub mod notice;
mod segmented;
pub mod spinner;
mod status_label;
mod trigger;
mod window_chrome;

use lurq::app::theme::Theme;

pub(super) fn install(theme: &Theme) {
  avatar::install(theme);
  button::install(theme);
  callout::install(theme);
  card::install(theme);
  detail_row::install(theme);
  chip::install(theme);
  icon_button::install(theme);
  kbd::install(theme);
  meter::install(theme);
  notice::install(theme);
  segmented::install(theme);
  spinner::install(theme);
  status_label::install(theme);
  trigger::install(theme);
  window_chrome::install(theme);
}
