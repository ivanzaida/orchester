//! Messages of the chat. An agent's (`Agent message` `dcd0m`): a 24 avatar (`surface_raised` in a `border`
//! stroke, radius 6, a 13 glyph in `text_secondary`), 10 from who wrote it (12/600) with its step and time
//! (11 `text_muted`) and under them, 3 apart, what it wrote, as every chat draws it (`ui::chat::agent_text`); then
//! its tool groups ([`super::chips`]) at the chat's edge. A person's (`Your message` `mr4lC`): at the right, *You*
//! and the time, the message as written in a bubble that hugs it up to 80 % of the chat (`own_message` in a
//! `border` stroke, radius 10, 8 by 12 inside) and its delivery line (a 12 icon and 11 `text_muted`, 5 apart):
//! sending now, queued and until when, delivered mid-turn or at the end of a turn (`check-check` in `success`), or
//! not delivered, kept, with the composer's actions under it (`composer::undelivered`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  conversation::{Arrival, Undelivered},
  time::Timestamp,
};
use orchester_workspace::{
  display,
  tasks::{
    conversation::{Entry, EntryState, Queue},
    timeline::view::AgentRow,
  },
};

use super::{Timeline, chips};
pub(crate) use crate::ui::chat::links;
use crate::ui::{
  chat::{Avatar, agent_text, chat_row, delivery_line, person_message as bubble, person_side_message},
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

/// A person's message (`mr4lC`) at the right: *You* (or the Orchestrator, for one the project's Orchestrator chat
/// sent), its time, the bubble and its delivery line.
pub(in super::super) fn person_message(ctx: &mut Ctx, entry: &Entry) -> Element {
  let (glyph, color, words) = delivery(ctx, &entry.state);
  let delivery = delivery_line("conversation-delivery", (glyph, color), &words);
  let undelivered = super::super::composer::undelivered(ctx, entry);
  let id = format!("conversation-message-{}", entry.id);
  let message = if entry.by_orchestrator {
    let author = ctx.t("conversation.byOrchestrator");
    person_side_message(&id, (&author, &entry.body, Some(entry.at)), Some(delivery))
  } else if entry.by_automation {
    let author = ctx.t("conversation.byAutomation");
    person_side_message(&id, (&author, &entry.body, Some(entry.at)), Some(delivery))
  } else {
    bubble(ctx, &id, (&entry.body, Some(entry.at)), Some(delivery))
  };
  message
    .with_children(undelivered)
    .describe("delivery", delivery_name(&entry.state))
    .describe("by_orchestrator", entry.by_orchestrator.to_string())
    .into()
}

/// A note the person returned the work with, as their message, without a delivery line.
pub(super) fn person_note(
  ctx: &mut Ctx,
  id: &str,
  note: &Arc<str>,
  at: Option<Timestamp>,
  by: Option<orchester_core::run::Answerer>,
) -> Element {
  if by == Some(orchester_core::run::Answerer::Automation) {
    let author = ctx.t("conversation.byAutomation");
    return person_side_message(id, (&author, note, at), None).into();
  }
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
