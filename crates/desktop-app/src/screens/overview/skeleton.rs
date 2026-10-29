//! The Overview while the project's tasks are read (04 `A3qRTI`, and `01 · Overview · loading` `ae6PR`): the header,
//! the sections' heads and their actions are real; each section's rows are bones in their own geometry (a status dot,
//! the key, the title, and what is at the row's end), shown after 300 ms so a fast reading never flashes them, pulsing
//! unless reduced motion is asked for (`ui::skeleton`). Plan limits come from memory and draw at once.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::SpacingSize,
  },
  components::{Column, Row, Spacer},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::SKELETON_DELAY,
  ui::skeleton::{bone, pulsing, round_bone},
};

/// A row's bones: the status dot, the key and the end (`Row bone` in 04).
const DOT: f32 = 14.0;
const KEY: (f32, f32) = (40.0, 9.0);
const TITLE_HEIGHT: f32 = 9.0;
const END: (f32, f32) = (52.0, 9.0);

/// Bones for `rows` rows of `height`, their titles as wide as `titles` says in turn, under `id`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct BonesProps {
  pub(super) id: &'static str,
  pub(super) height: f32,
  pub(super) titles: &'static [f32],
}

impl DevtoolsInspectable for BonesProps {}

pub(super) fn bones(ctx: &mut Ctx, props: BonesProps) -> Element {
  ctx.mount_keyed::<Bones>(props.id, props)
}

struct Bones {
  shown: Signal<bool>,
  /// Shows the bones once the reading took [`SKELETON_DELAY`].
  _delay: Timeout,
}

impl Component for Bones {
  type Props = BonesProps;

  fn create(ctx: &mut Ctx) -> Self {
    let shown = ctx.signal(false);
    let show = shown.clone();
    let delay = ctx.create_timeout(SKELETON_DELAY, move || show.set(true));
    delay.start();
    Self { shown, _delay: delay }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<BonesProps>().clone();
    let shown = self.shown.get();
    let frame = Column::new()
      .id(props.id)
      .width(Dimension::Pct(100.0))
      .describe("state", if shown { "skeleton" } else { "waiting" });
    if !shown {
      return frame;
    }
    let rows = props.titles.iter().map(|title| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(props.height))
        .padding(Padding::horizontal(SpacingSize::Sm))
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(round_bone(DOT))
        .child(bone(KEY.0, KEY.1))
        .child(bone(*title, TITLE_HEIGHT))
        .child(Spacer::new().flex(1.0))
        .child(bone(END.0, END.1))
    });
    frame.child(pulsing(Column::new().width(Dimension::Pct(100.0)).with_children(rows)))
  }
}
