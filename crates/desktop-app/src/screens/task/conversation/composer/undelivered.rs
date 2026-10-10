//! Under a message sent now that its agent's CLI did not take (*Could not deliver to Builder: … Your message
//! is kept here.*): sending it again, which queues it for when the agent's turn ends and tells the runner
//! of its own run, offered while it is a message of the latest run and an agent can still read it, and
//! named for what happens then (*Send after this turn* while an agent works, *Send as follow-up* once the
//! run is ready for review, *Send when the run continues* while it waits); and *Edit in composer*, which
//! puts its words into the composer after what is written there, with the focus there, offered while the
//! composer takes words (handover, "Messages"). A message put back into the composer offers nothing more. What the
//! runner did not take the composer says under its box; why the message was not queued again is said here.

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  layout::Alignment,
  node::Element,
};
use orchester_core::id::MessageId;
use orchester_workspace::tasks::conversation::{Entry, EntryState, PanelState, outgoing::after_turn_run};

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

/// What sending the message again is called while the run stands at `panel`.
const fn again_key(panel: PanelState) -> &'static str {
  match panel {
    PanelState::Live => "composer.undelivered.afterTurn",
    PanelState::Ready => "composer.undelivered.followUp",
    _ => "composer.undelivered.whenContinues",
  }
}

fn buttons(ctx: &Ctx, props: &UndeliveredProps, composing: &Composing, pending: bool) -> Option<Row> {
  if composing.state.handled.get().0.contains(&props.message) {
    return None;
  }
  let reads = composing.panel.takes_messages();
  let after_turn = after_turn_run(&composing.detail, reads, props.message).map(|run| {
    let button = ActionButton::new(
      format!("composer-send-after-turn-{}", props.message),
      ButtonKind::Ghost,
      ctx.t(again_key(composing.panel)),
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
    let (state, message, text) = (composing.state.clone(), props.message, props.text.clone());
    ActionButton::new(
      format!("composer-edit-{}", props.message),
      ButtonKind::Ghost,
      ctx.t("composer.undelivered.edit"),
    )
    .icon(Icon::Pencil)
    .on_click(move || state.edit(message, &text))
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
