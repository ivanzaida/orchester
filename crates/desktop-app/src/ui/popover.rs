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
  components::{Column, Placement},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension, padding::Padding},
  ui,
};

use crate::{shell::status_bar::STATUS_BAR_HEIGHT, theme::ExtraSpacing};

/// Where a popover opens: under its control or over it, from the control's start, or under it from its end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopoverSide {
  Below,
  Above,
  BelowEnd,
}

impl PopoverSide {
  /// lurq's placement of this side; lurq counts a popup's offset away from its anchor on every side.
  const fn placement(self) -> Placement {
    match self {
      Self::Below => Placement::BottomStart,
      Self::Above => Placement::TopStart,
      Self::BelowEnd => Placement::BottomEnd,
    }
  }
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
    .background(PaletteColor::Card)
    .border_inside(BorderSize::Sm, PaletteColor::Input)
    .box_shadow(ShadowStyle::Lg)
}

/// `panel` opened 6 from `anchor` on `side` while `open` holds, in lurq's `ui::Popover`, whose own panel draws nothing
/// (`theme::kit::popover`): the kit's panel is the popover's. The focus goes to the first control in it as it opens and
/// back as it closes; a press outside or Esc closes it.
pub fn popover(anchor: ElementRef, open: Signal<bool>, side: PopoverSide, panel: impl Into<Element>) -> ui::Popover {
  ui::Popover::new(open, anchor)
    .placement(side.placement())
    .offset(ExtraSpacing::Snug.role())
    .child(panel)
}

/// `panel` opened 6 from `anchor` on `side` while `open` holds, which a press on `anchor` never opens: the app alone
/// opens it (a card's question), and a press of the anchor only closes it, as a press elsewhere does. It is
/// [`popover`] otherwise.
pub fn popup_by_app(
  anchor: ElementRef,
  open: Signal<bool>,
  side: PopoverSide,
  panel: impl Into<Element>,
) -> ui::Popover {
  popover(anchor, open, side, panel).switch_on_anchor_press(false)
}

/// `panel` opened 6 under `anchor` from its start while `open` holds, moved back just enough that it ends `end_inset`
/// or more before `frame` does (a column it belongs in): lurq's `within_inset` with the `Clamp` collision. It never
/// flips over its anchor: give it [`room_under`] as its most height.
pub fn popover_inside(
  (anchor, frame): (&ElementRef, &ElementRef),
  open: Signal<bool>,
  end_inset: f32,
  panel: impl Into<Element>,
) -> ui::Popover {
  popover(anchor.clone(), open, PopoverSide::Below, panel)
    .within_inset(frame.clone(), Padding::new().right(end_inset))
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
/// item's popover, `QqBCj`): 9 above the bar and its end 12 from the bar's, so it stays inside the window. The item
/// stays its trigger, where the focus goes back.
pub fn bar_popover(
  ctx: &Ctx,
  (item, bar): (&ElementRef, &ElementRef),
  open: Signal<bool>,
  panel: impl Into<Element>,
) -> ui::Popover {
  let inset = ctx.theme().spacing_value(SpacingSize::Md).to_px();
  ui::Popover::new(open, item.clone())
    .placed_by(bar.clone())
    .placement(Placement::TopEnd)
    .offset(ExtraSpacing::Inline.role())
    .cross_offset(-inset)
    .child(panel)
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
