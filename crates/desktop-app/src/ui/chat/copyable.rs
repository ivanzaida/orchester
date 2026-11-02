//! Messages of a chat with Copy ([`super::copy`]), which shows while the pointer is on the message. An agent's or the
//! Orchestrator's row has it at the end of its name line, which is `control_height_sm` high so the message does not move
//! when it shows (`Name` `GXvO5` in 07 `GgXmd`). A person's message, which the ORC-323 frames do not draw, has it
//! beside its bubble, at the bubble's start and bottom, 8 from it.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::Row,
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};

use super::{Avatar, copy::message_actions, head, message_row};
use crate::theme::ControlHeight;

/// The row `id` of a message whose Markdown is `markdown`: the avatar, its name line with `name`, `meta`, `mark` and
/// Copy, and `body` under it.
pub(super) fn row(
  ctx: &mut Ctx,
  id: String,
  (avatar, markdown): (Avatar, &Arc<str>),
  (name, meta, mark): (&str, &str, Option<Element>),
  body: Vec<Element>,
) -> Element {
  let props = RowProps {
    id: Arc::from(id.as_str()),
    avatar,
    name: Arc::from(name),
    meta: Arc::from(meta),
    marked: mark.is_some(),
    markdown: markdown.clone(),
  };
  let children = mark.into_iter().chain(body).collect();
  ctx.mount_keyed_with::<CopyableRow>(&format!("copyable-{id}"), props, children)
}

/// The bubble of the person's message `id` that says `words`, with Copy beside it.
pub(super) fn bubble(ctx: &mut Ctx, id: &str, words: &Arc<str>, bubble: Element) -> Element {
  let props = BubbleProps {
    id: Arc::from(id),
    words: words.clone(),
  };
  ctx.mount_keyed_with::<CopyableBubble>(&format!("copyable-{id}"), props, vec![bubble])
}

#[derive(Clone, Debug, PartialEq)]
struct RowProps {
  id: Arc<str>,
  avatar: Avatar,
  name: Arc<str>,
  meta: Arc<str>,
  /// The first child is the mark on the name line; the rest is the body.
  marked: bool,
  markdown: Arc<str>,
}

impl DevtoolsInspectable for RowProps {}

struct CopyableRow {
  hovered: Signal<bool>,
}

impl Component for CopyableRow {
  type Props = RowProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let mut children = ctx.children().to_vec();
    let mark = (props.marked && !children.is_empty()).then(|| children.remove(0));
    let actions = message_actions(ctx, &props.id, &props.markdown, self.hovered.get());
    let head = head((&props.name, &props.meta, mark))
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ControlHeight::Sm.value()))
      .child(Row::new().flex(1.0))
      .child(actions);
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    message_row(props.id.to_string(), props.avatar, head, children)
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
  }
}

#[derive(Clone, Debug, PartialEq)]
struct BubbleProps {
  id: Arc<str>,
  words: Arc<str>,
}

impl DevtoolsInspectable for BubbleProps {}

struct CopyableBubble {
  hovered: Signal<bool>,
}

impl Component for CopyableBubble {
  type Props = BubbleProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<BubbleProps>().clone();
    let bubble = ctx.children().to_vec();
    let actions = message_actions(ctx, &props.id, &props.words, self.hovered.get());
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    // The row is as wide as the chat, so the bubble keeps its widest of it; Copy sits at the bubble's start.
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::End)
      .justify(Justify::End)
      .child(actions)
      .with_children(bubble)
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
  }
}
