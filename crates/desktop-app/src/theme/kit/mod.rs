//! The recipes of `lurq::ui`'s components that the kit (`ui/`) draws, patched to the design's values: each kit
//! primitive that renders a lurq component keeps its look here.

pub mod avatar;
mod button;
pub mod callout;
pub mod card;
pub mod chip;
pub mod detail_row;
mod field;
mod hover_card;
mod icon_button;
mod kbd;
pub mod meter;
pub mod notice;
mod segmented;
mod skeleton;
pub mod spinner;
mod status_label;
mod tooltip;
mod trigger;
mod window_chrome;
pub mod window_tabs;

use lurq::app::theme::Theme;

pub(super) fn install(theme: &Theme) {
  avatar::install(theme);
  button::install(theme);
  callout::install(theme);
  card::install(theme);
  detail_row::install(theme);
  field::install(theme);
  chip::install(theme);
  hover_card::install(theme);
  icon_button::install(theme);
  kbd::install(theme);
  meter::install(theme);
  notice::install(theme);
  segmented::install(theme);
  skeleton::install(theme);
  spinner::install(theme);
  status_label::install(theme);
  tooltip::install(theme);
  trigger::install(theme);
  window_chrome::install(theme);
  window_tabs::install(theme);
}
