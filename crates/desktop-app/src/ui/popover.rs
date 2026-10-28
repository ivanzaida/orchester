//! Popover: a panel of settings or details that opens anchored to the control that opens it and closes
//! on a press outside or Esc (`Run permissions popover` `ojhrZ`): `surface_raised` inside a
//! `border_strong` stroke, radius 10, 14 over and under its content and 16 at its sides, its parts 12
//! apart, lifted by the `lg` shadow. It opens 6 under its control, or over it where the design puts it
//! above; one for a control at a card's end opens under it from the control's end; one that must stay in its
//! column opens under its control and inside the column (`Sources popover` `B56QE`).

use lurq::{
  app::{
    ctx::{CollisionStrategy, Ctx},
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize},
  },
  components::{Column, Placement, Popup},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{shell::status_bar::STATUS_BAR_HEIGHT, theme::ExtraSpacing};

/// Where a popover opens: under its control or over it, from the control's start, or under it from its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopoverSide {
  Below,
  Above,
  BelowEnd,
}

/// The panel of a popover `width` wide with the element id `id`; add its content as children.
pub fn popover_panel(id: &str, width: f32) -> Column {
  padded_popover_panel(
    id,
    width,
    Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()),
  )
}

/// [`popover_panel`] with `padding` of its own instead of the popover's, where a side may have none (a popover whose
/// scroll area reaches its edge).
pub fn padded_popover_panel(id: &str, width: f32, padding: Padding) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Px(width))
    .padding(padding)
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
    PopoverSide::BelowEnd => (Placement::BottomEnd, gap),
  };
  Popup::new(anchor, panel)
    .open(open)
    .placement(placement)
    .offset(0.0, offset)
}

/// `panel`, `width` wide, opened under `anchor` from its start while `open` holds, moved back just enough that it ends
/// `end_inset` or more before `frame` does (a column it belongs in), as both were last laid out. It never flips over
/// its anchor: give it [`room_under`] as its most height.
pub fn popover_inside(
  (anchor, frame): (&ElementRef, &ElementRef),
  open: Signal<bool>,
  (width, end_inset): (f32, f32),
  panel: impl Into<Element>,
) -> Popup {
  let past = (anchor.x() + width) - (frame.x() + frame.width() - end_inset);
  Popup::new(anchor.clone(), panel)
    .open(open)
    .placement(Placement::BottomStart)
    .offset(-past.max(0.0), ExtraSpacing::Snug.value())
    .collision(CollisionStrategy::Clamp)
}

/// How tall a popover under `anchor` may be: the room between it, the popover's gap under it, and what sits at the
/// window's bottom (the status bar, and `above_bar` over it, such as a page's action bar), kept 12 clear; `None` before
/// the anchor was laid out.
pub fn room_under(ctx: &Ctx, anchor: &ElementRef, above_bar: f32) -> Option<f32> {
  let margin = ctx.theme().spacing_value(SpacingSize::Md).to_px();
  let bottom = ctx.window().info().logical_height() - STATUS_BAR_HEIGHT - above_bar - margin;
  let top = anchor.y() + anchor.height() + ExtraSpacing::Snug.value();
  (anchor.height() > 0.0).then(|| (bottom - top).max(0.0))
}

/// `panel` opened by the status bar item `item` while `open` holds, placed over the status bar `bar` (the MCP
/// item's popover, `QqBCj`): 9 above the bar and its end 12 from the bar's, so it stays inside the window.
pub fn bar_popover(
  ctx: &Ctx,
  (item, bar): (&ElementRef, &ElementRef),
  open: Signal<bool>,
  panel: impl Into<Element>,
) -> Popup {
  let inset = ctx.theme().spacing_value(SpacingSize::Md).to_px();
  let (x, y) = at_frame_end(item, bar, PopoverSide::Above);
  Popup::new(item.clone(), panel)
    .open(open)
    .placement(Placement::TopEnd)
    // Over its anchor, lurq counts the offset away from it.
    .offset(x - inset, y + ExtraSpacing::Inline.value())
}

/// How far a popup at the end of `trigger`, over or under it, moves to open at the end of `frame`, the element
/// around the trigger the design places it by (the status bar, a segmented control), as both were last laid out.
/// The trigger stays the popup's anchor: lurq switches to a popup in one press, from another open dropdown, only
/// when the pressed control is the anchor itself.
pub fn at_frame_end(trigger: &ElementRef, frame: &ElementRef, side: PopoverSide) -> (f32, f32) {
  let x = (frame.x() + frame.width()) - (trigger.x() + trigger.width());
  let y = match side {
    PopoverSide::Above => trigger.y() - frame.y(),
    PopoverSide::Below | PopoverSide::BelowEnd => (frame.y() + frame.height()) - (trigger.y() + trigger.height()),
  };
  (x, y)
}
