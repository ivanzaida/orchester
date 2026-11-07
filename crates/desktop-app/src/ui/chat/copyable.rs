//! Messages of a chat with their actions ([`super::copy`]), which show while the pointer is on the message
//! (`06 · Message actions` `XPDAX`, ORC-706). An agent's or the Orchestrator's row has them in a line under what it
//! wrote, 8 under it, starting 4 before its text so a glyph stands near the text's edge (`Line` `SRzsZ`); its name
//! line keeps only who wrote it and when. A person's message has them in the line under its bubble, at its end, with
//! where the message stands after them, last, so nothing moves when they show (`Line` `yLkjx`). The line keeps its
//! room while it is hidden.

use std::sync::{Arc, OnceLock};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::{
  AVATAR_SIZE, Avatar,
  copy::{Edge, message_actions},
  head, message_row,
  toolbar::MessageExtras,
};
use crate::theme::{ControlHeight, ExtraSpacing};

/// How far an answer's actions start before its text: the glyph in its 24 button then stands near the text's edge
/// (`Line` `SRzsZ`, padding -4).
const LINE_OUTDENT: f32 = 4.0;

/// The row `id` of a message whose Markdown is `markdown`, as `super::chat_row_marked` draws a message, with Copy in a
/// line under it while the pointer is on it ([`super::copy`]).
pub fn row(
  ctx: &mut Ctx,
  id: &str,
  (avatar, markdown): (Avatar, &Arc<str>),
  (name, meta, mark): (&str, &str, Option<Element>),
  body: Vec<Element>,
) -> Element {
  row_with(ctx, id, (avatar, markdown, None), (name, meta, mark), body)
}

/// [`row`] whose line holds `extras` around Copy.
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

/// The person's message `id` that says `words`: its bubble and the files attached to it (`parts`, either may be
/// missing), then the line under them, its actions with `extras` around Copy and `delivery`, where the message stands,
/// last.
pub(super) fn person(
  ctx: &mut Ctx,
  id: &str,
  (words, extras): (&Arc<str>, Option<MessageExtras>),
  parts: [Option<Element>; 2],
  delivery: Option<Element>,
) -> Element {
  let props = PersonProps {
    id: Arc::from(id),
    words: words.clone(),
    extras,
    delivered: delivery.is_some(),
  };
  let children = parts.into_iter().flatten().chain(delivery).collect();
  ctx.mount_keyed_with::<PersonMessage>(&format!("copyable-{id}"), props, children)
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

/// Whether the pointer is on a row: the signal of its [`RowActions`], which own it so that a change draws only them.
type Pointer = Arc<OnceLock<Signal<bool>>>;

struct CopyableRow {
  pointer: Pointer,
}

impl Component for CopyableRow {
  type Props = RowProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self {
      pointer: Arc::new(OnceLock::new()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let mut children = ctx.children().to_vec();
    let mark = (props.marked && !children.is_empty()).then(|| children.remove(0));
    let actions = ctx.mount_keyed::<RowActions>(
      &format!("row-actions-{}", props.id),
      ActionsProps {
        id: props.id.clone(),
        markdown: props.markdown.clone(),
        extras: props.extras.clone(),
        pointer: self.pointer.clone(),
      },
    );
    let head = head((&props.name, &props.meta, mark));
    // The line stands outside the row, which clips what leaves it, so it can start before the text. It is as high as
    // its buttons before they are mounted, so the row's height never changes under a list that measured it.
    let indent = AVATAR_SIZE + ExtraSpacing::Loose.value() - LINE_OUTDENT;
    let (enter, leave) = (self.pointer.clone(), self.pointer.clone());
    Column::new()
      .id(props.id.to_string())
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(message_row(
        format!("{}-message", props.id),
        props.avatar,
        head,
        children,
      ))
      .child(
        Row::new()
          .height(Dimension::Px(ControlHeight::Sm.value()))
          .padding(Padding::default().left(indent))
          .child(actions),
      )
      .on_mouse_enter(move || enter.get().into_iter().for_each(|on| on.set(true)))
      .on_mouse_leave(move || leave.get().into_iter().for_each(|on| on.set(false)))
  }
}

/// What [`RowActions`] shows: the row's actions; `pointer` gets their signal of the pointer on the row.
#[derive(Clone)]
struct ActionsProps {
  id: Arc<str>,
  markdown: Arc<str>,
  extras: Option<MessageExtras>,
  pointer: Pointer,
}

/// Compared by what it shows; the pointer's cell is the row's own for as long as it lives.
impl PartialEq for ActionsProps {
  fn eq(&self, other: &Self) -> bool {
    (&self.id, &self.markdown, &self.extras) == (&other.id, &other.markdown, &other.extras)
  }
}

impl DevtoolsInspectable for ActionsProps {}

/// The actions of a [`CopyableRow`], which own the signal of the pointer on the row, so that the pointer coming onto
/// it draws only them. A signal dirties the component that made it, and the row's body comes as slot children: a
/// render of the row would put them back as its parent last drew them, over what a component in the body drew for the
/// same pointer (a waits-on line's *Drop the wait*, `lguJb`).
struct RowActions {
  hovered: Signal<bool>,
}

impl Component for RowActions {
  type Props = ActionsProps;

  fn create(ctx: &mut Ctx) -> Self {
    let hovered = ctx.signal(false);
    // The row's cell takes the first signal; its actions are mounted under one key for as long as it lives.
    let _ = ctx.props::<ActionsProps>().pointer.set(hovered.clone());
    Self { hovered }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ActionsProps>().clone();
    message_actions(
      ctx,
      &props.id,
      (&props.markdown, props.extras),
      (Edge::Start, self.hovered.get()),
    )
  }
}

#[derive(Clone, PartialEq)]
struct PersonProps {
  id: Arc<str>,
  words: Arc<str>,
  extras: Option<MessageExtras>,
  /// The last child is where the message stands.
  delivered: bool,
}

impl DevtoolsInspectable for PersonProps {}

struct PersonMessage {
  hovered: Signal<bool>,
}

impl Component for PersonMessage {
  type Props = PersonProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<PersonProps>().clone();
    let mut parts = ctx.children().to_vec();
    let delivery = props.delivered.then(|| parts.pop()).flatten();
    let actions = message_actions(
      ctx,
      &props.id,
      (&props.words, props.extras.clone()),
      (Edge::End, self.hovered.get()),
    );
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::End)
      .with_children(parts)
      .child(
        Row::new()
          .id(format!("{}-meta", props.id))
          .height(Dimension::Px(ControlHeight::Sm.value()))
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(actions)
          .with_children(delivery),
      )
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
  }
}
