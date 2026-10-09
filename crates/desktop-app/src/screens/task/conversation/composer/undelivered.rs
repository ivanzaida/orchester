//! Under a message sent now that its agent's CLI did not take (*Could not deliver to Builder: … Your message
//! is kept here.*): *Send after this turn*, which queues it again for when the agent's turn ends, and *Edit
//! in composer*, which puts its words back into the composer with the focus there (handover, "Messages").

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  layout::Alignment,
  node::Element,
};
use orchester_core::id::MessageId;
use orchester_workspace::tasks::conversation::{Entry, EntryState};

use super::{
  Composing,
  send::{AfterTurn, AfterTurnAction, after_turn_action},
};
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

  fn create(ctx: &mut Ctx) -> Self {
    let action = after_turn_action(ctx);
    let feed = composing(ctx).project.feed;
    ctx.watch(&action.state(), move |state| {
      if !state.is_pending() && state.data.is_some() {
        feed.saved();
      }
    });
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let action = after_turn_action(ctx);
    let props = ctx.props::<UndeliveredProps>().clone();
    let composing = composing(ctx);
    let state = action.state().get();
    let pending = state.is_pending();
    let error = state.error.filter(|_| !pending).map(|error| {
      let text = ctx.t_args("composer.undelivered.failed", [("reason", &*message_text(ctx, &error))]);
      field_error(&format!("composer-undelivered-failed-{}", props.message), &text)
    });
    Column::new()
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::End)
      .child(buttons(ctx, &props, &composing, (&action, pending)))
      .with_children(error)
  }
}

/// What the message's actions act on, as the tab provides it.
fn composing(ctx: &mut Ctx) -> Composing {
  ctx
    .use_context::<Composing>()
    .expect("the Conversation tab provides the composer before its chat")
}

fn buttons(
  ctx: &Ctx,
  props: &UndeliveredProps,
  composing: &Composing,
  (action, pending): (&AfterTurnAction, bool),
) -> Row {
  let mut after_turn = ActionButton::new(
    format!("composer-send-after-turn-{}", props.message),
    ButtonKind::Ghost,
    ctx.t("composer.undelivered.afterTurn"),
  )
  .icon(Icon::Clock);
  match composing.detail.run.as_ref() {
    Some(run) if !pending => {
      let request = AfterTurn {
        project: composing.project.open.clone(),
        task: composing.detail.task.id(),
        run: run.id(),
        message: props.message,
      };
      let action = action.clone();
      after_turn = after_turn.on_click(move || action.run(request.clone()));
    }
    Some(_) => after_turn = after_turn.loading(),
    // A message its agent did not take belongs to a run; without one there is nothing to send it to.
    None => after_turn = after_turn.disabled(),
  }
  let (link, text) = (composing.link.clone(), props.text.clone());
  let edit = ActionButton::new(
    format!("composer-edit-{}", props.message),
    ButtonKind::Ghost,
    ctx.t("composer.undelivered.edit"),
  )
  .icon(Icon::Pencil)
  .on_click(move || link.edit(text.clone()));
  Row::new().spacing(SpacingSize::Sm).child(after_turn).child(edit)
}
