//! Messages of the chat. An agent's (`Agent message` `dcd0m`): a 24 avatar (`surface_raised` in a `border`
//! stroke, radius 6, a 13 glyph in `text_secondary`), 10 from who wrote it (12/600) with its step and time
//! (11 `text_muted`) and under them, 3 apart, what it wrote, as every chat draws it (`ui::chat::agent_text`); then
//! its tool groups ([`super::chips`]) at the chat's edge. A person's (`Your message` `mr4lC`): at the right, *You*
//! and the time, the message as written in a bubble that hugs it up to 80 % of the chat (`own_message` in a
//! `border` stroke, radius 10, 8 by 12 inside) and its delivery line (a 12 icon and 11 `text_muted`, 5 apart):
//! sending now, queued and until when, delivered mid-turn or at the end of a turn (`check-check` in `success`), or
//! not delivered, kept, with the composer's actions under it (`composer::undelivered`). One the project's
//! Orchestrator chat sent (ORC-45) is the Orchestrator's row, as an agent's is drawn, with how it went out.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  conversation::{Arrival, Undelivered},
  id::ProjectId,
  orchestrator::FromOrchestrator,
  time::Timestamp,
};
use orchester_workspace::{
  display,
  tasks::{
    conversation::{Entry, EntryState, Queue},
    timeline::view::AgentRow,
  },
};

use super::{super::super::orchestrated::sent_line, Timeline, chips};
pub(crate) use crate::ui::chat::links;
use crate::ui::{
  chat::{Avatar, agent_text, chat_row, delivery_line, person_message as bubble},
  icon::Icon,
};

/// An agent's row: its message, if it wrote one, then the tool groups after it at the chat's edge.
pub(super) fn agent_row(ctx: &mut Ctx, timeline: &Timeline, row: &AgentRow) -> Element {
  let time = display::clock(row.at);
  let meta = match (timeline.step_of(ctx, row.execution), row.tool_step) {
    (_, true) => ctx.t_args("conversation.toolMeta", [("time", time.as_str())]),
    (Some(step), false) => ctx.t_args("conversation.agentMeta", [("step", &*step), ("time", time.as_str())]),
    (None, false) => time.as_str().into(),
  };
  let avatar = if row.tool_step { Avatar::Tool } else { Avatar::Agent };
  let message = row.text.as_ref().map(|said| {
    let on_link = links(ctx);
    let body = vec![agent_text(
      ctx,
      &format!("conversation-text-{}", row.key),
      said,
      (timeline.project, &timeline.names),
      &on_link,
    )];
    chat_row(
      format!("conversation-agent-{}", row.key),
      avatar,
      (&row.agent, &meta),
      body,
    )
  });
  let groups = chips::groups(ctx, timeline, &row.chips);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .with_children(message)
    .with_children(groups)
    .into()
}

/// A person's message (`mr4lC`) at the right: *You*, its time, the bubble and its delivery line. One the project's
/// Orchestrator chat sent is the Orchestrator's instead ([`orchestrator_message`]).
pub(in super::super) fn person_message(ctx: &mut Ctx, (project, entry): (ProjectId, &Entry)) -> Element {
  let (glyph, color, words) = delivery(ctx, &entry.state);
  let delivery = delivery_line("conversation-delivery", (glyph, color), &words);
  let undelivered = super::super::composer::undelivered(ctx, entry);
  let id = format!("conversation-message-{}", entry.id);
  let message = match entry.orchestrator {
    Some(from) => orchestrator_message(ctx, &id, (project, entry, from), delivery),
    None => bubble(ctx, &id, (&entry.body, Some(entry.at)), Some(delivery)),
  };
  message
    .with_children(undelivered)
    .describe("delivery", delivery_name(&entry.state))
    .describe("by_orchestrator", entry.orchestrator.is_some().to_string())
    .into()
}

/// A message the project's Orchestrator chat sent the run's agents, at the chat's side as its rows are, never in the
/// person's bubble: the Orchestrator's `waypoints` avatar and name with the time, the message as it was sent, how it
/// went out with the way back to the chat's conversation ([`sent_line`]), and its delivery line as the person's.
fn orchestrator_message(
  ctx: &mut Ctx,
  id: &str,
  (project, entry, from): (ProjectId, &Entry, FromOrchestrator),
  delivery: Element,
) -> Column {
  let name = ctx.t("conversation.byOrchestrator");
  let time = display::clock(entry.at);
  let said = Text::new(&entry.body)
    .id(format!("{id}-text"))
    .variant(TypographyStyle::Body)
    .selectable(true)
    .width(Dimension::Pct(100.0));
  let body: Vec<Element> = std::iter::once(said.into())
    .chain(sent_line(ctx, id, (project, from)))
    .chain(std::iter::once(delivery))
    .collect();
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(chat_row(
      format!("{id}-row"),
      Avatar::Orchestrator,
      (&name, &time),
      body,
    ))
}

/// A note the person returned the work with, as their message, without a delivery line.
pub(super) fn person_note(ctx: &mut Ctx, id: &str, note: &Arc<str>, at: Option<Timestamp>) -> Element {
  bubble(ctx, id, (note, at), None).into()
}

/// The name `describe` reports a delivery by, for tests and QA.
const fn delivery_name(state: &EntryState) -> &'static str {
  match state {
    EntryState::Queued(_) => "queued",
    EntryState::Delivered { .. } => "delivered",
    EntryState::NotDelivered { .. } => "not-delivered",
    EntryState::Reply { .. } => "reply",
    EntryState::Question { .. } => "question",
  }
}

fn delivery(ctx: &Ctx, state: &EntryState) -> (Icon, PaletteColor, Arc<str>) {
  let muted = PaletteColor::TextMuted;
  match state {
    EntryState::Queued(queue) => queued(ctx, queue),
    EntryState::Delivered {
      agent,
      at,
      arrival,
      sent_now,
    } => {
      let time = display::clock(*at);
      let args = [("agent", &**agent), ("time", time.as_str())];
      let key = match (arrival, sent_now) {
        (Arrival::MidTurn, _) => "conversation.delivery.midTurn",
        (Arrival::TurnEnd, true) => "conversation.delivery.turnEndedFirst",
        (Arrival::TurnEnd, false) => "conversation.delivery.turnEnd",
        (Arrival::Prompt, _) => "conversation.delivery.delivered",
      };
      (Icon::CheckCheck, PaletteColor::Success, ctx.t_args(key, args))
    }
    EntryState::NotDelivered { agent, reason } => {
      let reason = ctx.t(match reason {
        Undelivered::Refused => "conversation.delivery.refused",
        Undelivered::Unwritable => "conversation.delivery.unwritable",
      });
      let words = ctx.t_args(
        "conversation.delivery.notDelivered",
        [("agent", &**agent), ("reason", &*reason)],
      );
      (Icon::CircleAlert, PaletteColor::TextPrimary, words)
    }
    EntryState::Reply { step } => (
      Icon::Check,
      muted,
      ctx.t_args("conversation.delivery.reply", [("step", &**step)]),
    ),
    EntryState::Question { open: true } => (Icon::MessageCircle, muted, ctx.t("conversation.delivery.question")),
    EntryState::Question { open: false } => (Icon::Check, muted, ctx.t("conversation.delivery.answered")),
  }
}

fn queued(ctx: &Ctx, queue: &Queue) -> (Icon, PaletteColor, Arc<str>) {
  let agent = |key: &str, agent: &str| ctx.t_args(key, [("agent", agent)]);
  let (glyph, words) = match queue {
    Queue::SendingNow(name) => (Icon::LoaderCircle, agent("conversation.delivery.sendingNow", name)),
    Queue::AfterTurn(name) => (Icon::Clock, agent("conversation.delivery.afterTurn", name)),
    Queue::NextCheckpoint => (Icon::Clock, ctx.t("conversation.delivery.queued")),
    Queue::RunStarts => (Icon::Clock, ctx.t("conversation.delivery.queuedRunStart")),
    Queue::RunResumes(Some(name)) => (Icon::Clock, agent("conversation.delivery.queuedRunResumeBy", name)),
    Queue::RunResumes(None) => (Icon::Clock, ctx.t("conversation.delivery.queuedRunResume")),
    Queue::RunContinues(Some(name)) => (Icon::Clock, agent("conversation.delivery.queuedRunContinueBy", name)),
    Queue::RunContinues(None) => (Icon::Clock, ctx.t("conversation.delivery.queuedRunContinue")),
    Queue::AgentNextTurn(name) => (Icon::Clock, agent("conversation.delivery.queuedAgentNextTurn", name)),
    Queue::NextRun => (Icon::Clock, ctx.t("conversation.delivery.queuedNextRun")),
    Queue::RunEnded => (Icon::CircleX, ctx.t("conversation.delivery.notDeliveredEnded")),
    Queue::WorktreeRemoved => (Icon::CircleX, ctx.t("conversation.delivery.notDeliveredWorktree")),
  };
  (glyph, PaletteColor::TextMuted, words)
}
