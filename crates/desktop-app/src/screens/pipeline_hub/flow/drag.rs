//! Dragging a card by its handle (`Drag handle` `yylYY`, the dragged `Dragged card · 05 Review` `c7u7qB` in P3
//! `K4B56`): the card lifts once the pointer moved [`LIFT`] with the button held, so a press on the handle never moves
//! a card. Its place stays faded; a copy, tilted and lifted by a shadow, follows the pointer; where it is over decides
//! where it lands ([`super::landing`]); letting go drops it through the edit, Esc sets it down.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::{DragEvent, MouseButtonMask, MouseEvent},
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize},
  },
  components::{Button, Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{
    CursorIcon, Element, HitTestBehavior, border::Border, dimension::Dimension, padding::Padding,
    transform::Transform2D,
  },
};
use orchester_core::resource::StepKey;
use orchester_workspace::pipelines::flow::StepCard;

use super::{
  reorder::{Drive, Move},
  state::{Drag, Drop, Reorder},
  words,
};
use crate::{
  theme::{ExtraSpacing, FlowText, IconSize},
  ui::icon::{Icon, icon},
};

/// How far the pointer moves with the button held before a card lifts.
const LIFT: f32 = 4.0;
/// How far the floating card is tilted (`Dragged card · 05 Review`).
const TILT: f32 = -1.5;
/// The floating card's width, whatever the card's own (`Dragged card · 05 Review` `c7u7qB`).
const LIFTED_WIDTH: f32 = 240.0;
/// A card's line under its name is indented past the handle and its gap.
const INDENT: f32 = 20.0;

/// The handle of `card`, which lifts it; `card_ref` is the card's element.
pub(super) fn handle(ctx: &Ctx, drive: &Drive, card: &StepCard, card_ref: ElementRef) -> Element {
  let (moving, ending, started) = (drive.clone(), drive.clone(), drive.reorder.clone());
  let (stepped, key) = (card.clone(), card.key.clone());
  let id = format!("pipeline-hub-flow-step-{}-handle", card.key);
  Button::empty()
    .id(id.clone())
    .focusable(false)
    .cursor(CursorIcon::Grab)
    .describe(
      "a11y_name",
      ctx.t_args("pipeline_hub.flow.move.handle", [("name", card.name.as_ref())]),
    )
    .on_click(|event: MouseEvent| event.stop_propagation())
    .start_drag_buttons(MouseButtonMask::LEFT)
    .on_drag_start(move |_: DragEvent| *started.set_down.lock() = false)
    .on_drag_move(move |event: DragEvent| moved(&moving.reorder, (&stepped, &card_ref), &event))
    .on_drag_end(move |_: DragEvent| ended(&ending, &key))
    .child(icon(Icon::GripVertical, IconSize::Md, PaletteColor::TextSecondary).id(format!("{id}-icon")))
    .into()
}

/// The pointer moved with the handle held: once it went [`LIFT`], the card lifts; then the flow follows where it is
/// over.
fn moved(reorder: &Reorder, (card, card_ref): (&StepCard, &ElementRef), event: &DragEvent) {
  if *reorder.set_down.lock() || *reorder.locked.lock() {
    return;
  }
  let lifted = reorder.drag.get_untracked();
  if lifted.is_none() && event.total_delta_x.hypot(event.total_delta_y) < LIFT {
    return;
  }
  reorder.pointer.set((event.x, event.y));
  let over = reorder.drop_at((event.x, event.y));
  match lifted {
    None => {
      let bounds = card_ref.bounds();
      reorder.refused.set(None);
      reorder.drag.set(Some(Drag {
        key: card.key.clone(),
        card: card.clone(),
        over,
        grab: (event.start_x - bounds.x, event.start_y - bounds.y),
      }));
    }
    Some(drag) if drag.over != over => reorder.drag.set(Some(Drag { over, ..drag })),
    Some(_) => {}
  }
}

/// The button was let go: a lifted card drops where it is over, unless Esc set it down.
fn ended(drive: &Drive, key: &StepKey) {
  let reorder = &drive.reorder;
  let set_down = std::mem::take(&mut *reorder.set_down.lock());
  let Some(drag) = reorder.drag.get_untracked() else {
    return;
  };
  reorder.drag.set(None);
  let Some(over) = drag.over.filter(|_| !set_down) else {
    return;
  };
  let change = match over {
    Drop::Before(before) => Move::Before(before),
    Drop::OnCard(target) => Move::Beside(target),
  };
  drive.apply(key, &change, false);
}

/// Esc: sets a lifted card down, which ends its drag without a drop. Whether there was one.
pub(super) fn set_down(reorder: &Reorder) -> bool {
  if reorder.drag.get_untracked().is_none() {
    return false;
  }
  *reorder.set_down.lock() = true;
  reorder.drag.set(None);
  true
}

#[derive(Clone)]
pub(super) struct LiftedProps {
  pub drag: Drag,
  pub pointer: Signal<(f32, f32)>,
  pub frame: ElementRef,
}

impl PartialEq for LiftedProps {
  fn eq(&self, other: &Self) -> bool {
    self.drag == other.drag
  }
}
impl DevtoolsInspectable for LiftedProps {}

/// The card a drag lifted, under the pointer: it follows the pointer in a render of its own, so the flow is drawn again
/// only when what the card is over changes.
pub(super) struct Lifted;

impl Component for Lifted {
  type Props = LiftedProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<LiftedProps>().clone();
    let (x, y) = props.pointer.get();
    let frame = props.frame.bounds();
    lifted_card(ctx, &props.drag.card, LIFTED_WIDTH).absolute(
      x - frame.x - props.drag.grab.0.min(LIFTED_WIDTH),
      y - frame.y - props.drag.grab.1,
      Dimension::Px(LIFTED_WIDTH),
      Dimension::Auto,
    )
  }
}

/// `Dragged card · 05 Review`: `surface_raised` in a `border_strong` stroke, lifted by the `md` shadow and tilted, its
/// handle, number and name over what it runs.
fn lifted_card(ctx: &Ctx, card: &StepCard, width: f32) -> Column {
  let top = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::GripVertical, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(&words::number(card.number))
        .variant(FlowText::CardNumber.role())
        .nowrap(),
    )
    .child(Text::new(&card.name).variant(FlowText::Name.role()).nowrap());
  Column::new()
    .id("pipeline-hub-flow-lifted")
    .width(Dimension::Px(width))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .bottom(SpacingSize::Sm)
        .right(SpacingSize::Md)
        .left(ExtraSpacing::Snug.role()),
    )
    .spacing(SpacingSize::Xs)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .box_shadow(ShadowStyle::Md)
    .transform(Transform2D::rotate_deg(TILT))
    .hit_test(HitTestBehavior::None)
    .child(top)
    .child(
      Row::new().padding(Padding::new().left(Dimension::Px(INDENT))).child(
        Text::new(&words::runs(ctx, &card.runs))
          .variant(FlowText::Line.role())
          .nowrap(),
      ),
    )
}
