//! Messages of the chat. An agent's (`Agent message` `dcd0m`): a 24 avatar (`surface_raised` in a `border`
//! stroke, radius 6, a 13 glyph in `text_secondary`), 10 from who wrote it (12/600) with its step and time
//! (11 `text_muted`) and under them, 3 apart, what it wrote, as every chat draws it (`ui::chat::agent_text`); then
//! its tool groups ([`super::chips`]) at the chat's edge. A person's (`Your message` `mr4lC`): at the right, *You*
//! and the time, the message as written in a bubble that hugs it up to 80 % of the chat (`own_message` in a
//! `border` stroke, radius 10, 8 by 12 inside) and its delivery line (a 12 icon and 11 `text_muted`, 5 apart):
//! sending now, queued and until when, delivered mid-turn or at the end of a turn (`check-check` in `success`), or
//! not delivered, kept, with the composer's actions under it (`composer::undelivered`). One the project's
//! Orchestrator chat sent (ORC-45) is the Orchestrator's row, as an agent's is drawn, with how it went out.

use std::{path::Path, sync::Arc};

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
  pull_request::ci::fix::FixedBy,
  time::Timestamp,
};
use orchester_workspace::{
  display,
  files::Version,
  tasks::{
    conversation::{Entry, EntryState, Queue},
    timeline::view::AgentRow,
  },
};

use super::{super::super::orchestrated::sent_line, Timeline, chips};
pub(crate) use crate::ui::chat::links;
use crate::ui::{
  attachments,
  chat::{
    self, Avatar, agent_text, chat_row, copyable_row, delivery_line, found::Found, marked_words,
    person_message as bubble, person_side_message,
  },
  icon::Icon,
};

/// An agent's row: its message with its Copy, if it wrote one, what the search found in it filled behind its words,
/// then the tool groups after it at the chat's edge.
pub(super) fn agent_row(ctx: &mut Ctx, timeline: &Timeline, row: &AgentRow) -> Element {
  let time = display::clock(row.at);
  let meta = match (timeline.step_of(ctx, row.execution), row.tool_step) {
    (_, true) => ctx.t_args("conversation.toolMeta", [("time", time.as_str())]),
    (Some(step), false) => ctx.t_args("conversation.agentMeta", [("step", &*step), ("time", time.as_str())]),
    (None, false) => time.as_str().into(),
  };
  let avatar = if row.tool_step { Avatar::Tool } else { Avatar::Agent };
  let found = timeline.search.as_ref().and_then(|marks| marks.in_said(&row.key));
  let message = row.text.as_ref().map(|said| {
    let on_link = links(ctx);
    let body = vec![agent_text(
      ctx,
      &format!("conversation-text-{}", row.key),
      said,
      (timeline.project, &timeline.names),
      (&on_link, found.as_ref()),
    )];
    copyable_row(
      ctx,
      &format!("conversation-agent-{}", row.key),
      (avatar, said),
      (&row.agent, &meta, None),
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

/// A person's message (`mr4lC`) at the right: *You* (or *Automation*, for one automation wrote), its time, the bubble,
/// the files attached to it (`ui::attachments::sent`, kept in the project at `root`) and its delivery line; a file it
/// mentions opens at `files_at`. One the project's Orchestrator chat sent is the Orchestrator's instead
/// ([`orchestrator_message`]); the chat attaches no files. What a search found in it (`found`) is filled behind its
/// words. A message a Fix CI sent (`fixed`, ORC-335) is its Markdown, each log tail a code block, and says so once its
/// agent took it: *Fix CI · sent 15:44*, or *Sent automatically · Fix CI 2 of 3* with `repeat`.
pub(in super::super) fn person_message(
  ctx: &mut Ctx,
  ((project, root, files_at), entry): ((ProjectId, &Path, &Version), &Entry),
  (found, fixed): (Option<&Found>, Option<FixedBy>),
) -> Element {
  let (glyph, color, words) = match (fixed, &entry.state) {
    (Some(by), EntryState::Delivered { .. }) => fix_delivery(ctx, by, entry.at),
    _ => delivery(ctx, &entry.state),
  };
  let delivery = delivery_line("conversation-delivery", (glyph, color), &words);
  let undelivered = super::super::composer::undelivered(ctx, entry);
  let id = format!("conversation-message-{}", entry.id);
  let attached = attachments::sent::sent(ctx, &id, root, &entry.attachments);
  // The person's words, with their mentions as links and without the facts the agent got with them; a Fix CI's
  // Markdown as the chat draws an agent's, its log tails code blocks (`11b` `X8xYZ`).
  let mut said = || {
    if entry.attachments_only() {
      None
    } else if fixed.is_some() {
      chat::said::composed(ctx, &id, (&entry.body, found), project)
    } else {
      chat::said::said(ctx, &id, (&entry.body, found), (project, files_at))
    }
  };
  let message = match entry.orchestrator {
    Some(from) => orchestrator_message(ctx, &id, (project, entry, from), (delivery, found)),
    None if entry.by_automation => {
      let said = said();
      let author = ctx.t("conversation.byAutomation");
      person_side_message(ctx, &id, (&author, said, Some(entry.at)), (attached, Some(delivery)))
    }
    None => {
      let said = said();
      bubble(ctx, &id, (said, Some(entry.at)), (attached, Some(delivery)))
    }
  };
  message
    .with_children(undelivered)
    .describe("delivery", delivery_name(&entry.state))
    .describe("by_orchestrator", entry.orchestrator.is_some().to_string())
    .describe("attachments", entry.attachments.len().to_string())
    .into()
}

/// A message the project's Orchestrator chat sent the run's agents, at the chat's side as its rows are, never in the
/// person's bubble: the Orchestrator's `waypoints` avatar and name with the time, the message as it was sent, how it
/// went out with the way back to the chat's conversation ([`sent_line`]), and its delivery line as the person's.
fn orchestrator_message(
  ctx: &mut Ctx,
  id: &str,
  (project, entry, from): (ProjectId, &Entry, FromOrchestrator),
  (delivery, found): (Element, Option<&Found>),
) -> Column {
  orchestrator_row(
    ctx,
    id,
    (project, from),
    (Some(&entry.body), Some(entry.at), found),
    Some(delivery),
  )
}

/// The Orchestrator's row `id`: its avatar and name with the time `at`, what it wrote (`said`) with what a search
/// found in it filled behind its words (`found`) and its Copy, how that went out with the way back to the chat's
/// conversation, and `after` under them.
fn orchestrator_row(
  ctx: &mut Ctx,
  id: &str,
  (project, from): (ProjectId, FromOrchestrator),
  (said, at, found): (Option<&str>, Option<Timestamp>, Option<&Found>),
  after: Option<Element>,
) -> Column {
  let name = ctx.t("conversation.byOrchestrator");
  let time = at.map(display::clock).unwrap_or_default();
  let shown = said.map(|said| {
    let words = Text::new(said)
      .id(format!("{id}-text"))
      .variant(TypographyStyle::Body)
      .selectable(true)
      .width(Dimension::Pct(100.0));
    marked_words(ctx, words, said, found)
  });
  let body: Vec<Element> = shown
    .into_iter()
    .chain(sent_line(ctx, id, (project, from)))
    .chain(after)
    .collect();
  let row_id = format!("{id}-row");
  let row = match said {
    Some(said) => copyable_row(
      ctx,
      &row_id,
      (Avatar::Orchestrator, &Arc::from(said)),
      (&name, &time, None),
      body,
    ),
    None => chat_row(row_id, Avatar::Orchestrator, (&name, &time), body),
  };
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(row)
}

/// A note the person returned the work with, as their message, without a delivery line; what a search found in it
/// (`found`) is filled behind its words.
pub(super) fn person_note(
  ctx: &mut Ctx,
  id: &str,
  (note, found): (&Arc<str>, Option<&Found>),
  at: Option<Timestamp>,
  by: Option<orchester_core::run::Answerer>,
) -> Element {
  let said = chat::plain_said(ctx, id, note, found);
  if by == Some(orchester_core::run::Answerer::Automation) {
    let author = ctx.t("conversation.byAutomation");
    return person_side_message(ctx, id, (&author, said, at), (None, None)).into();
  }
  bubble(ctx, id, (said, at), (None, None)).into()
}

/// How the Orchestrator chat went on with the run (ORC-45): its row, with the note the work returned with when there
/// is one, never the person's bubble; what a search found in the note (`found`) is filled behind its words.
pub(super) fn orchestrator_note(
  ctx: &mut Ctx,
  id: &str,
  (project, from): (ProjectId, FromOrchestrator),
  (note, at, found): (Option<&Arc<str>>, Option<Timestamp>, Option<&Found>),
) -> Element {
  orchestrator_row(ctx, id, (project, from), (note.map(|note| &**note), at, found), None)
    .describe("by_orchestrator", "true")
    .into()
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

/// The delivery line of a message a Fix CI sent `by` at `at`, once its agent took it.
fn fix_delivery(ctx: &Ctx, by: FixedBy, at: Timestamp) -> (Icon, PaletteColor, Arc<str>) {
  let time = display::clock(at);
  match by {
    FixedBy::Hand => (
      Icon::CheckCheck,
      PaletteColor::TextMuted,
      ctx.t_args("ci.message.sent", [("time", time)]),
    ),
    FixedBy::Automatically { count, limit } => (
      Icon::Repeat,
      PaletteColor::TextMuted,
      ctx.t_args(
        "ci.message.sentAuto",
        [
          ("count", count.to_string()),
          ("limit", limit.get().to_string()),
          ("time", time),
        ],
      ),
    ),
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
