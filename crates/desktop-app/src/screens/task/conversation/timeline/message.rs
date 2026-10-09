//! Messages of the chat. An agent's (`Agent message` `dcd0m`): a 24 avatar (`surface_raised` in a `border`
//! stroke, radius 6, a 13 glyph in `text_secondary`), 10 from who wrote it (12/600) with its step and time
//! (11 `text_muted`) and under them, 3 apart, what it wrote at 13/1.5, read as Markdown; then its tool groups
//! ([`super::chips`]) at the chat's edge. A person's (`Your message` `mr4lC`): at the right, *You* and the
//! time, the message as written in a bubble that hugs it up to 80 % of the chat (`own_message` in a `border`
//! stroke, radius 10, 8 by 12 inside) and its delivery line (a 12 icon and 11 `text_muted`, 5 apart): sending
//! now, queued and until when, delivered mid-turn or at the end of a turn (`check-check` in `success`), or not
//! delivered, kept.

use std::sync::{Arc, LazyLock};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  conversation::{Arrival, Undelivered},
  time::Timestamp,
};
use orchester_exec::external;
use orchester_workspace::{
  display,
  tasks::{
    conversation::{Entry, EntryState, Queue},
    timeline::view::AgentRow,
  },
};

use super::{Timeline, chips};
use crate::{
  theme::{ConversationText, ExtraColor, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    markdown::{LinkHandler, MarkdownView},
  },
};

/// The avatar's box (`HO4bJ`) and how wide a person's bubble may grow, of the chat's width.
const AVATAR_SIZE: f32 = 24.0;
const BUBBLE_WIDTH: f32 = 80.0;

/// A press on a message's link opens it in the browser; one handler for every message, so a message is laid
/// out again only when what it says changes.
static OPEN_LINK: LazyLock<LinkHandler> = LazyLock::new(|| Arc::new(open_link));

/// What a press on a link in the chat does.
pub(super) fn links() -> LinkHandler {
  OPEN_LINK.clone()
}

fn open_link(destination: &str) {
  if let Err(error) = external::open_url(destination) {
    tracing::warn!(%error, "the message's link was not opened");
  }
}

/// Who a row is from, as its avatar shows it: an agent, or a command tool that runs a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Avatar {
  Agent,
  Tool,
}

/// The avatar, then `name` and `meta` with `body` under them; `id` names the row.
pub(super) fn chat_row(id: String, avatar: Avatar, (name, meta): (&str, &str), body: Vec<Element>) -> Element {
  let glyph = match avatar {
    Avatar::Agent => Icon::Bot,
    Avatar::Tool => Icon::Terminal,
  };
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .child(
      Row::new()
        .size(Dimension::Px(AVATAR_SIZE), Dimension::Px(AVATAR_SIZE))
        .flex_shrink(0.0)
        .background(PaletteColor::SurfaceRaised)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Md)
        .align_items(Alignment::Center)
        .justify(Justify::Center)
        .child(icon(glyph, IconSize::Base, PaletteColor::TextSecondary)),
    )
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(
          Row::new()
            .spacing(ExtraSpacing::Snug.role())
            .align_items(Alignment::Center)
            .child(
              Text::new(name)
                .id("conversation-message-name")
                .variant(ConversationText::Name.role())
                .nowrap(),
            )
            .child(
              Text::new(meta)
                .id("conversation-message-meta")
                .variant(TypographyStyle::Caption)
                .nowrap(),
            ),
        )
        .with_children(body),
    )
    .into()
}

/// What a message says, its Markdown rendered; `id` names it.
pub(super) fn text(ctx: &mut Ctx, id: &str, text: &Arc<str>) -> Element {
  MarkdownView::new(id, text.clone(), links()).render(ctx)
}

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
    let body = vec![text(ctx, &format!("conversation-text-{}", row.key), said)];
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

/// A person's message (`mr4lC`) at the right: *You*, its time, the bubble and its delivery line.
pub(in super::super) fn person_message(ctx: &mut Ctx, entry: &Entry) -> Element {
  let delivery = delivery_line(ctx, &entry.state);
  bubble_column(
    ctx,
    &format!("conversation-message-{}", entry.id),
    (&entry.body, Some(entry.at)),
    Some(delivery),
  )
  .describe("delivery", delivery_name(&entry.state))
  .into()
}

/// A note the person returned the work with, as their message, without a delivery line.
pub(super) fn person_note(ctx: &mut Ctx, id: &str, note: &Arc<str>, at: Option<Timestamp>) -> Element {
  bubble_column(ctx, id, (note, at), None).into()
}

/// *You*, the time when it is known, the bubble and `delivery`.
fn bubble_column(
  ctx: &mut Ctx,
  id: &str,
  (body, at): (&Arc<str>, Option<Timestamp>),
  delivery: Option<Element>,
) -> Column {
  let head = Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("conversation.you"))
        .variant(TypographyStyle::FieldLabel)
        .nowrap(),
    )
    .with_children(at.map(|at| {
      Text::new(&display::clock(at))
        .variant(TypographyStyle::Caption)
        .nowrap()
    }));
  // As the person wrote it, so the bubble hugs its words up to its widest (`pCzqz`): lurq's Markdown always
  // fills its parent's width.
  let said = Text::new(body)
    .id(format!("{id}-text"))
    .variant(TypographyStyle::Body)
    .selectable(true);
  let bubble = Column::new()
    .id(format!("{id}-bubble"))
    .max_width(Dimension::Pct(BUBBLE_WIDTH))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .background(ExtraColor::OwnMessage.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(said);
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::End)
    .child(head)
    .child(bubble)
    .with_children(delivery)
}

/// Where a person's message stands, with its icon.
fn delivery_line(ctx: &Ctx, state: &EntryState) -> Element {
  let (glyph, color, words) = delivery(ctx, state);
  let strong = color == PaletteColor::TextPrimary;
  let words = Text::new(&words).variant(TypographyStyle::Caption);
  Row::new()
    .id("conversation-delivery")
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Sm, color.clone()))
    .child(if strong { words.color(color) } else { words })
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
