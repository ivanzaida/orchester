//! Popover: a panel of settings or details that opens anchored to the control that opens it and closes
//! on a press outside or Esc (`Run permissions popover` `ojhrZ`): `surface_raised` inside a
//! `border_strong` stroke, radius 10, 14 over and under its content and 16 at its sides, its parts 12
//! apart, lifted by the `lg` shadow. It opens 6 under its control, or over it where the design puts it
//! above.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize},
  },
  components::{Column, Placement, Popup},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::theme::ExtraSpacing;

/// Where a popover opens: under its control or over it, from the control's start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopoverSide {
  Below,
  Above,
}

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

/// `panel` opened on `side` of `anchor` while `open` holds; a press outside or Esc closes it.
pub fn popover(anchor: ElementRef, open: Signal<bool>, side: PopoverSide, panel: impl Into<Element>) -> Popup {
  let gap = ExtraSpacing::Snug.value();
  let (placement, offset) = match side {
    PopoverSide::Below => (Placement::BottomStart, gap),
    PopoverSide::Above => (Placement::TopStart, -gap),
  };
  Popup::new(anchor, panel)
    .open(open)
    .placement(placement)
    .offset(0.0, offset)
}

/// `panel` over the status bar `bar` while `open` holds, for an item at the bar's end (the MCP item's popover,
/// `QqBCj`): 9 above the bar and its end 12 from the bar's, so it stays inside the window.
pub fn bar_popover(ctx: &Ctx, bar: ElementRef, open: Signal<bool>, panel: impl Into<Element>) -> Popup {
  let inset = ctx.theme().spacing_value(SpacingSize::Md).to_px();
  Popup::new(bar, panel)
    .open(open)
    .placement(Placement::TopEnd)
    // Over its anchor, lurq counts the offset away from it.
    .offset(-inset, ExtraSpacing::Inline.value())
}
