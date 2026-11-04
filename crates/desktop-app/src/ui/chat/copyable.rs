//! Messages of a chat with Copy ([`super::copy`]), which shows while the pointer is on the message. An agent's or the
//! Orchestrator's row has it at the end of its name line, which is `control_height_sm` high so the message does not
//! move when it shows (`Name` `GXvO5` in 07 `GgXmd`). A person's message, which the ORC-323 frames do not draw, has it
//! beside its bubble, at the bubble's start and bottom, 8 from it; one whose toolbar has more than Copy
//! ([`super::toolbar`], ORC-330) has it at the start of the line under the bubble, 8 before where the message stands
//! (`Meta · hover` `A5WAyl` in 09 `GYAPg`). That toolbar keeps its room while it is hidden, as Copy's does.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};

use super::{Avatar, copy::message_actions_with, head, message_row, toolbar::MessageExtras};
use crate::theme::ControlHeight;

/// The row `id` of a message whose Markdown is `markdown`, as `super::chat_row_marked` draws a message, with Copy at
/// the end of its name line while the pointer is on it ([`super::copy`]); that line is `control_height_sm` high, with
/// or without Copy.
pub fn row(
  ctx: &mut Ctx,
  id: &str,
  (avatar, markdown): (Avatar, &Arc<str>),
  (name, meta, mark): (&str, &str, Option<Element>),
  body: Vec<Element>,
) -> Element {
  row_with(ctx, id, (avatar, markdown, None), (name, meta, mark), body)
}

/// [`row`] whose toolbar holds `extras` around Copy.
pub fn row_with(
  ctx: &mut Ctx,
  id: &str,
  (avatar, markdown, extras): (Avatar, &Arc<str>, Option<MessageExtras>),
  (name, meta, mark): (&str, &str, Option<Element>),
  body: Vec<Element>,
) -> Element {
  let props = RowProps {
    id: Arc::from(id),
    avatar,
    name: Arc::from(name),
    meta: Arc::from(meta),
    marked: mark.is_some(),
    markdown: markdown.clone(),
    extras,
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

/// The person's message `id` that says `words`: its bubble and the files attached to it (`parts`, either may be
/// missing), then the line under them, its toolbar with `extras` at its start and `delivery`, where the message
/// stands, after it.
pub(super) fn acted(
  ctx: &mut Ctx,
  id: &str,
  (words, extras): (&Arc<str>, MessageExtras),
  parts: [Option<Element>; 2],
  delivery: Element,
) -> Element {
  let props = ActedProps {
    id: Arc::from(id),
    words: words.clone(),
    extras,
  };
  let children = parts.into_iter().flatten().chain([delivery]).collect();
  ctx.mount_keyed_with::<ActedMessage>(&format!("copyable-{id}"), props, children)
}

#[derive(Clone, PartialEq)]
struct RowProps {
  id: Arc<str>,
  avatar: Avatar,
  name: Arc<str>,
  meta: Arc<str>,
  /// The first child is the mark on the name line; the rest is the body.
  marked: bool,
  markdown: Arc<str>,
  extras: Option<MessageExtras>,
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
    let actions = message_actions_with(
      ctx,
      &props.id,
      (&props.markdown, props.extras.clone()),
      self.hovered.get(),
    );
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
    let actions = message_actions_with(ctx, &props.id, (&props.words, None), self.hovered.get());
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

#[derive(Clone, PartialEq)]
struct ActedProps {
  id: Arc<str>,
  words: Arc<str>,
  extras: MessageExtras,
}

impl DevtoolsInspectable for ActedProps {}

struct ActedMessage {
  hovered: Signal<bool>,
}

impl Component for ActedMessage {
  type Props = ActedProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ActedProps>().clone();
    let mut parts = ctx.children().to_vec();
    let delivery = parts.pop();
    let actions = message_actions_with(
      ctx,
      &props.id,
      (&props.words, Some(props.extras.clone())),
      self.hovered.get(),
    );
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    Column::new()
      .id(props.id.to_string())
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::End)
      .with_children(parts)
      .child(
        Row::new()
          .id(format!("{}-meta", props.id))
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(actions)
          .with_children(delivery),
      )
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
  }
}
