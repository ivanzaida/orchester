//! Under a message sent now that its agent's CLI did not take (*Could not deliver to Builder: … Your message
//! is kept here.*): *Send after this turn*, which queues it again for when the agent's turn ends and tells
//! the runner of its own run, offered while it is a message of the latest run and an agent can still read
//! it; and *Edit in composer*, which puts its words into the composer after what is written there, with the
//! focus there, offered while the composer takes words (handover, "Messages"). What the runner did not take
//! the composer says under its box; why the message was not queued again is said here.

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  layout::Alignment,
  node::Element,
};
use orchester_core::id::{MessageId, RunId};
use orchester_workspace::tasks::conversation::{Entry, EntryState};

use super::{Composing, state::AfterTurn};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  field::field_error,
  icon::Icon,
  message::message_text,
};

/// The actions under `entry` when its agent's CLI did not take it; `None` for any other message.
pub(in super::super) fn undelivered(ctx: &mut Ctx, entry: &Entry) -> Option<Element> {
  if !matches!(entry.state, EntryState::NotDelivered { .. }) {
    return None;
  }
  Some(ctx.mount_keyed::<Undelivered>(
    &format!("composer-undelivered-{}", entry.id),
    UndeliveredProps {
      message: entry.id,
      text: entry.body.to_string(),
    },
  ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UndeliveredProps {
  message: MessageId,
  text: String,
}

impl lurq::app::component::DevtoolsInspectable for UndeliveredProps {}

struct Undelivered;

impl Component for Undelivered {
  type Props = UndeliveredProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<UndeliveredProps>().clone();
    let composing = ctx
      .use_context::<Composing>()
      .expect("the Conversation tab provides the composer before its chat");
    let state = composing.state.after_turn.state().get();
    let (pending, rejected) = (state.is_pending(), state.is_rejected());
    let error = state
      .error
      .filter(|refused| !pending && rejected && refused.message == props.message)
      .map(|refused| {
        let text = ctx.t_args(
          "composer.undelivered.failed",
          [("reason", &*message_text(ctx, &refused.reason))],
        );
        field_error(&format!("composer-undelivered-failed-{}", props.message), &text)
      });
    let buttons = buttons(ctx, &props, &composing, pending);
    Column::new()
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::End)
      .with_children(buttons)
      .with_children(error)
  }
}

/// The run the message was sent to, while it is the task's latest and an agent of it can still read it.
fn readable_run(composing: &Composing, message: MessageId) -> Option<RunId> {
  let latest = composing.detail.run.as_ref()?.id();
  let run = composing
    .detail
    .messages
    .iter()
    .find(|stored| stored.id() == message)?
    .run()?;
  (run == latest && composing.takes_messages).then_some(run)
}

fn buttons(ctx: &Ctx, props: &UndeliveredProps, composing: &Composing, pending: bool) -> Option<Row> {
  let after_turn = readable_run(composing, props.message).map(|run| {
    let button = ActionButton::new(
      format!("composer-send-after-turn-{}", props.message),
      ButtonKind::Ghost,
      ctx.t("composer.undelivered.afterTurn"),
    )
    .icon(Icon::Clock);
    if pending {
      return button.loading();
    }
    let request = AfterTurn {
      project: composing.project.open.clone(),
      task: composing.detail.task.id(),
      run,
      message: props.message,
    };
    let action = composing.state.after_turn.clone();
    button.on_click(move || action.run(request.clone()))
  });
  let edit = composing.writable.then(|| {
    let (state, text) = (composing.state.clone(), props.text.clone());
    ActionButton::new(
      format!("composer-edit-{}", props.message),
      ButtonKind::Ghost,
      ctx.t("composer.undelivered.edit"),
    )
    .icon(Icon::Pencil)
    .on_click(move || state.edit(&text))
  });
  if after_turn.is_none() && edit.is_none() {
    return None;
  }
  Some(
    Row::new()
      .spacing(SpacingSize::Sm)
      .with_children(after_turn)
      .with_children(edit),
  )
}
