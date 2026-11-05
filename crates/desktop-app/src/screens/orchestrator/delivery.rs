//! The person's message that opens a row, and where it stands under it (`Your message` `mr4lC`): *Sending…*, then
//! *Delivered* and when. A message written while the Orchestrator answered says how it reaches the answer (ORC-92):
//! queued after the answer, or next once it stops; handed to the answer and on its way; taken by the answer and when;
//! or not taken by the answer, so it goes after it. The design draws no queued message for the chat: these lines are
//! the delivery line's (`check-check` in `success` once it reached the Orchestrator, a `clock` in `text_muted` while it
//! waits), and a queued message offers *Edit*, which takes its words and files back into the composer, and *Remove*,
//! as the task's composer offers *Edit in composer* under a message its agent did not take.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::Row,
  node::Element,
};
use orchester_core::orchestrator::{Queued, TurnMode, TurnState};
use orchester_workspace::{display, files::Version};

use super::{send::QueuedChange, timeline::TurnRow, turns::Arrival};
use crate::ui::{
  attachments,
  button::{ActionButton, ButtonKind},
  chat::{delivery_line, own_bubble, own_bubble_with, said::said},
  icon::Icon,
};

/// The person's message that opens `row`'s turn, keyed `key`, with the files attached to it and where it stands; the
/// delivery line under it says when, after its actions. A message that reached the Orchestrator has *Edit*, Copy,
/// *Rewind to here* and *Fork from here* there (`super::rewind::toolbar`), any other Copy alone; while it is edited,
/// the field takes its place (`super::rewind::editing`). One that waits has *Edit* and *Remove* under that line.
pub(super) fn opening_message(ctx: &mut Ctx, row: &TurnRow, key: &str) -> Element {
  let id = format!("orchestrator-message-{key}");
  if let Some((_, after)) = row
    .env
    .rewind
    .editing()
    .filter(|(editing, _)| Some(editing.message) == row.turn.message)
  {
    return super::rewind::editing::field(ctx, &id, (&row.env.rewind, &row.env.open), after);
  }
  let delivery = delivery(ctx, row);
  let attached = attachments::sent::sent(ctx, &id, &row.env.root, &row.turn.attachments);
  // The person's words, with their mentions as links and without the facts the Orchestrator got with them; the files
  // they mention are the project's checkout's.
  let said = said(ctx, &id, (&row.turn.text, None), (row.env.project, &Version::Checkout));
  if let Some(extras) = row.env.rewind.on_message(ctx, &row.turn) {
    return own_bubble_with(ctx, &id, said, (attached, delivery), extras).into();
  }
  own_bubble(ctx, &id, said, (attached, Some(delivery)))
    .with_children(queued_actions(ctx, row))
    .into()
}

/// Where the message stands: its glyph, its color and its words.
fn delivery(ctx: &Ctx, row: &TurnRow) -> Element {
  let id = "orchestrator-delivery";
  let time = row.turn.at.map(display::clock).unwrap_or_default();
  let waiting = |key: &str| delivery_line(id, (Icon::Clock, PaletteColor::TextMuted), &ctx.t(key));
  match (row.turn.arrival, &row.turn.state, row.turn.at) {
    (Arrival::Waiting, Some(TurnState::Queued(Queued::AfterAnswer)), _) => waiting("orchestrator.delivery.queued"),
    (Arrival::Waiting, Some(TurnState::Queued(Queued::Next)), _) => waiting("orchestrator.delivery.queuedNext"),
    (Arrival::Waiting, Some(TurnState::Queued(Queued::NotTaken)), _) => waiting("orchestrator.delivery.notTaken"),
    (Arrival::Waiting, ..) => delivery_line(
      id,
      (Icon::LoaderCircle, PaletteColor::TextMuted),
      &ctx.t("orchestrator.delivery.steering"),
    ),
    (Arrival::Joined, ..) => delivery_line(
      id,
      (Icon::CheckCheck, PaletteColor::Success),
      &ctx.t_args("orchestrator.delivery.joined", [("time", time.as_str())]),
    ),
    (Arrival::Own, _, None) => delivery_line(
      id,
      (Icon::LoaderCircle, PaletteColor::TextMuted),
      &ctx.t("orchestrator.delivery.sending"),
    ),
    (Arrival::Own, _, Some(_)) => {
      // A message sent in plan-only mode says so (`12b` `X7BYz`).
      let key = if row.turn.mode == TurnMode::Plans {
        "orchestrator.plan.delivered"
      } else {
        "orchestrator.delivery.delivered"
      };
      delivery_line(
        id,
        (Icon::CheckCheck, PaletteColor::Success),
        &ctx.t_args(key, [("time", time.as_str())]),
      )
    }
  }
}

/// *Edit* and *Remove* under a message that waits in the queue, once it is kept; nothing for any other.
fn queued_actions(ctx: &Ctx, row: &TurnRow) -> Option<Row> {
  let queued = matches!(row.turn.state, Some(TurnState::Queued(_))) && row.turn.arrival == Arrival::Waiting;
  let message = row.turn.message.filter(|_| queued)?;
  let button = |change: QueuedChange, (suffix, label, glyph): (&str, &str, Icon)| {
    let (act, words) = (row.env.change_queued.clone(), row.turn.text.clone());
    ActionButton::new(
      format!("orchestrator-queued-{suffix}-{message}"),
      ButtonKind::Ghost,
      ctx.t(label),
    )
    .icon(glyph)
    .on_click(move || act(message, words.clone(), change))
  };
  Some(
    Row::new()
      .spacing(SpacingSize::Sm)
      .child(button(
        QueuedChange::Edit,
        ("edit", "orchestrator.queued.edit", Icon::Pencil),
      ))
      .child(button(
        QueuedChange::Remove,
        ("remove", "orchestrator.queued.remove", Icon::X),
      )),
  )
}
