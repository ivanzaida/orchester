//! Popover: a panel of settings or details that opens anchored to the control that opens it and closes
//! on a press outside or Esc (`Run permissions popover` `ojhrZ`): `surface_raised` inside a
//! `border_strong` stroke, radius 10, 14 over and under its content and 16 at its sides, its parts 12
//! apart, lifted by the `lg` shadow. It opens 6 from its control.

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize},
  components::{Column, Placement, Popup},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::theme::ExtraSpacing;

/// Space between a control and the popover it opens.
const POPOVER_GAP: f32 = 6.0;

/// The panel of a popover `width` wide with the element id `id`; add its content as children.
pub fn popover_panel(id: &str, width: f32) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Px(width))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Md)
    .rounded(RadiusSize::Lg)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
    .box_shadow(ShadowStyle::Lg)
}

/// `panel` opened at `placement` of `anchor` while `open` holds; a press outside or Esc closes it.
pub fn popover(anchor: ElementRef, open: Signal<bool>, placement: Placement, panel: impl Into<Element>) -> Popup {
  let gap = match placement {
    Placement::TopStart | Placement::Top | Placement::TopEnd => -POPOVER_GAP,
    _ => POPOVER_GAP,
  };
  Popup::new(anchor, panel)
    .open(open)
    .placement(placement)
    .offset(0.0, gap)
}
