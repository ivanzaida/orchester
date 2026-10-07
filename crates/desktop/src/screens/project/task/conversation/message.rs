//! Messages in the chat (`kpnmP`, [`super::chat`]): an agent's, with the chips of its tool activity
//! ([`super::chips`]); the person's, with its delivery line (icon 11 `$subtle`, text 10 `$muted`):
//! sending now, queued, delivered mid-turn or at the end of a turn, or not delivered, kept, with
//! *Send after this turn* and *Edit in composer*; a tool step's failing command with its output
//! (`FeiRG`). In *Tools* a message is one line (`q4fCXK`).

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  conversation::{Arrival, Undelivered},
  id::{RunId, TaskId},
};

use super::{
  super::activity::{EntryProps, EntryView},
  Timeline,
  chat::{Avatar, chat_row, chat_text},
  chips::chips,
  send,
};
use crate::{
  display,
  tasks::{
    conversation::{Author, Entry, EntryState, Queue},
    timeline::{Item, ItemKind, view::AgentRow},
    transcript::Entry as TranscriptEntry,
  },
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::field_error,
    icon::{Icon, IconSize, icon},
    markdown::plain::plain_text,
    message_text,
  },
};

/// The time column of a one-line entry (`N4SlQq`) and the glyph tile beside it.
const TIME_WIDTH: f32 = 36.0;
const GLYPH_SIZE: f32 = 24.0;

/// An agent's row: its message, if it wrote one, and the chips of its tool groups after it.
pub(super) fn agent_row(ctx: &mut Ctx, row: &AgentRow, timeline: &Timeline, raw: bool) -> Element {
  let time = display::clock(row.at);
  let meta = match (&row.step, row.tool_step) {
    (_, true) => ctx.t_args("runs.conversation.toolMeta", [("time", time.as_str())]),
    (Some(step), false) => ctx.t_args(
      "runs.conversation.agentMeta",
      [("step", &**step), ("time", time.as_str())],
    ),
    (None, false) => time.as_str().into(),
  };
  let avatar = if row.tool_step { Avatar::Tool } else { Avatar::Agent };
  let text = row
    .text
    .as_ref()
    .map(|text| chat_text(ctx, "conversation-message-text", text));
  let body = text.into_iter().chain(chips(ctx, timeline, &row.chips, raw)).collect();
  // A row of tool activity alone is told apart from a message.
  let id = if row.text.is_some() {
    "conversation-agent-message"
  } else {
    "conversation-agent-tools"
  };
  chat_row(id.into(), avatar, (&row.agent, &meta), body)
}

/// A tool step's command that failed the step (`FeiRG`): the step's own message, the command with
/// its exit code and output open in a bordered box.
pub(super) fn tool_step_failure(ctx: &mut Ctx, item: &Item, entry: &TranscriptEntry, timeline: &Timeline) -> Element {
  let step = timeline.step_name(entry.execution).unwrap_or_else(|| "".into());
  let meta = ctx.t_args(
    "runs.conversation.toolMeta",
    [("time", display::clock(item.at).as_str())],
  );
  let command = ctx.mount_keyed::<EntryView>(
    &item.key,
    EntryProps {
      key: item.key.clone(),
      task: timeline.task,
      entry: entry.clone(),
      agent: "".into(),
      raw: false,
      time_column: false,
      in_group: false,
      toggles: timeline.toggles.clone(),
      request: None,
    },
  );
  let frame = Column::new()
    .id("conversation-failed-command")
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Stack.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Notice.role())
    .child(command);
  chat_row(
    format!("conversation-tool-{}", item.key),
    Avatar::Tool,
    (&step, &meta),
    vec![frame.into()],
  )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PersonMessageProps {
  pub(super) entry: Entry,
  pub(super) repo_root: PathBuf,
  pub(super) task: TaskId,
  pub(super) run: RunId,
}

impl lurq::app::component::DevtoolsInspectable for PersonMessageProps {}

/// A person's message; one its agent's CLI did not take offers to send it after the turn.
pub(super) struct PersonMessageView;

impl Component for PersonMessageView {
  type Props = PersonMessageProps;

  fn create(ctx: &mut Ctx) -> Self {
    let action = send::after_turn_action(ctx);
    let reload = ctx
      .use_context::<Timeline>()
      .expect("the tab provides its timeline before its rows")
      .reload;
    ctx.watch(&action.state(), move |state| {
      if !state.is_pending() && state.data.is_some() {
        reload.update(|revision| *revision += 1);
      }
    });
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<PersonMessageProps>().clone();
    let action = send::after_turn_action(ctx);
    let timeline = ctx
      .use_context::<Timeline>()
      .expect("the tab provides its timeline before its rows");
    let entry = &props.entry;
    let state = action.state().get();
    let pending = state.is_pending();
    let actions = matches!(entry.state, EntryState::NotDelivered { .. })
      .then(|| not_delivered_actions(ctx, &props, (&action, pending), &timeline));
    let error = state
      .error
      .filter(|_| !pending)
      .map(|error| field_error(&message_text(ctx, &error)));
    person_message(ctx, entry, actions.into_iter().chain(error).collect())
  }
}

/// A person's message (`mjkqh` in `kpnmP`): *You*, whom it is to and when, what it says, its
/// delivery line and `below` it.
pub(super) fn person_message(ctx: &mut Ctx, entry: &Entry, below: Vec<Element>) -> Element {
  let time = display::clock(entry.at);
  let meta = match &entry.author {
    Author::YouTo(agent) => ctx.t_args(
      "runs.conversation.toAgent",
      [("agent", &**agent), ("time", time.as_str())],
    ),
    Author::You | Author::Agent(_) => ctx.t_args("runs.conversation.toOrchestrator", [("time", time.as_str())]),
  };
  let text = chat_text(ctx, &format!("conversation-text-{}", entry.id), &entry.body);
  let body = vec![text, delivery_line(ctx, &entry.state)]
    .into_iter()
    .chain(below)
    .collect();
  Column::new()
    .id(format!("conversation-message-{}", entry.id))
    .describe("state", format!("{:?}", entry.state))
    .width(Dimension::Pct(100.0))
    .child(chat_row(
      "conversation-person".into(),
      Avatar::Person,
      (&ctx.t("runs.conversation.you"), &meta),
      body,
    ))
    .into()
}

/// *Send after this turn* and *Edit in composer*, for a message the agent's CLI did not take.
fn not_delivered_actions(
  ctx: &Ctx,
  props: &PersonMessageProps,
  (action, pending): (&send::AfterTurnAction, bool),
  timeline: &Timeline,
) -> Element {
  let label = if pending {
    ctx.t("conversation.composer.sending")
  } else {
    ctx.t("runs.conversation.composer.sendAfterTurn")
  };
  let (action, request) = (
    action.clone(),
    send::AfterTurn {
      repo_root: props.repo_root.clone(),
      task: props.task,
      run: props.run,
      message: props.entry.id,
    },
  );
  let mut after_turn = ActionButton::new(label)
    .icon(Icon::Clock3)
    .compact()
    .id(format!("conversation-send-after-turn-{}", props.entry.id));
  after_turn = if pending {
    after_turn.disabled()
  } else {
    after_turn.on_click(move || action.run(request.clone()))
  };
  let (draft, focus, text) = (
    timeline.draft.clone(),
    timeline.focus.clone(),
    props.entry.body.to_string(),
  );
  let edit = ActionButton::new(ctx.t("runs.conversation.editInComposer"))
    .icon(Icon::Pencil)
    .compact()
    .id(format!("conversation-edit-in-composer-{}", props.entry.id))
    .on_click(move || {
      draft.set(text.clone());
      focus.set(true);
    });
  Row::new()
    .spacing(lurq::app::theme::SpacingSize::Sm)
    .child(after_turn.render(ctx))
    .child(edit.render(ctx))
    .into()
}

/// Where a person's message stands, with its icon: icon 11 `$subtle` and text 10 `$muted`; a
/// message that was not delivered in `$text`.
pub(super) fn delivery_line(ctx: &Ctx, state: &EntryState) -> Element {
  let (glyph, text, strong) = delivery(ctx, state);
  let color = if strong {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextMuted
  };
  let text = Text::new(&text).variant(ExtraTypography::Hint.role());
  Row::new()
    .id("conversation-delivery")
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Reason, color.clone()))
    .child(if strong { text.color(color) } else { text })
    .into()
}

fn delivery(ctx: &Ctx, state: &EntryState) -> (Icon, Arc<str>, bool) {
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
      let (glyph, key) = match (arrival, sent_now) {
        (Arrival::MidTurn, _) => (Icon::Zap, "runs.conversation.delivery.midTurn"),
        (Arrival::TurnEnd, true) => (Icon::Check, "runs.conversation.delivery.turnEndedFirst"),
        (Arrival::TurnEnd, false) => (Icon::Check, "runs.conversation.delivery.turnEnd"),
        (Arrival::Prompt, _) => (Icon::Check, "runs.conversation.delivery.delivered"),
      };
      (glyph, ctx.t_args(key, args), false)
    }
    EntryState::NotDelivered { agent, reason } => {
      let reason = ctx.t(match reason {
        Undelivered::Refused => "runs.conversation.delivery.refused",
        Undelivered::Unwritable => "runs.conversation.delivery.unwritable",
      });
      let text = ctx.t_args(
        "runs.conversation.delivery.notDelivered",
        [("agent", &**agent), ("reason", &*reason)],
      );
      (Icon::AlertCircle, text, true)
    }
    EntryState::Reply { step } => (
      Icon::Check,
      ctx.t_args("conversation.delivery.reply", [("step", &**step)]),
      false,
    ),
    EntryState::Question { open: true } => (Icon::MessageCircle, ctx.t("conversation.delivery.question"), false),
    EntryState::Question { open: false } => (Icon::Check, ctx.t("conversation.delivery.answered"), false),
  }
}

fn queued(ctx: &Ctx, queue: &Queue) -> (Icon, Arc<str>, bool) {
  let (glyph, text) = match queue {
    Queue::SendingNow(agent) => (
      Icon::Loader,
      ctx.t_args("runs.conversation.delivery.sendingNow", [("agent", &**agent)]),
    ),
    Queue::AfterTurn(agent) => (
      Icon::Clock3,
      ctx.t_args("runs.conversation.delivery.afterTurn", [("agent", &**agent)]),
    ),
    Queue::NextCheckpoint => (Icon::Clock3, ctx.t("conversation.delivery.queued")),
    Queue::RunStarts => (Icon::Clock3, ctx.t("conversation.delivery.queuedRunStart")),
    Queue::RunResumes(Some(agent)) => (
      Icon::Clock3,
      ctx.t_args("conversation.delivery.queuedRunResumeBy", [("agent", &**agent)]),
    ),
    Queue::RunResumes(None) => (Icon::Clock3, ctx.t("conversation.delivery.queuedRunResume")),
    Queue::RunContinues(Some(agent)) => (
      Icon::Clock3,
      ctx.t_args("conversation.delivery.queuedRunContinueBy", [("agent", &**agent)]),
    ),
    Queue::RunContinues(None) => (Icon::Clock3, ctx.t("conversation.delivery.queuedRunContinue")),
    Queue::AgentNextTurn(agent) => (
      Icon::Clock3,
      ctx.t_args("conversation.delivery.queuedAgentNextTurn", [("agent", &**agent)]),
    ),
    Queue::NextRun => (Icon::Clock3, ctx.t("conversation.delivery.queuedNextRun")),
    Queue::RunEnded => (Icon::CircleX, ctx.t("conversation.delivery.notDelivered")),
    Queue::WorktreeRemoved => (Icon::CircleX, ctx.t("conversation.delivery.notDeliveredWorktree")),
  };
  (glyph, text, false)
}

/// A message on one line in *Tools* (`q4fCXK`): its time, glyph, *Builder: …* or *You to Builder:
/// …*, and what it is or how it was delivered.
pub(super) fn one_line(ctx: &Ctx, item: &Item) -> Element {
  let (glyph, title, meta) = match &item.kind {
    ItemKind::AgentMessage { agent, text, .. } => (
      Icon::MessageSquare,
      ctx.t_args(
        "runs.conversation.line.agent",
        [("agent", &**agent), ("text", first_line(text).as_str())],
      ),
      ctx.t("runs.conversation.line.message"),
    ),
    ItemKind::Question { entry, .. } => (
      Icon::MessageCircle,
      ctx.t_args(
        "runs.conversation.line.agent",
        [
          ("agent", author_name(&entry.author)),
          ("text", first_line(&entry.body).as_str()),
        ],
      ),
      delivery(ctx, &entry.state).1,
    ),
    ItemKind::Message(entry) => {
      let title = match &entry.author {
        Author::YouTo(agent) => ctx.t_args(
          "runs.conversation.line.youTo",
          [("agent", &**agent), ("text", first_line(&entry.body).as_str())],
        ),
        Author::You | Author::Agent(_) => ctx.t_args(
          "runs.conversation.line.you",
          [("text", first_line(&entry.body).as_str())],
        ),
      };
      (Icon::User, title, delivery(ctx, &entry.state).1)
    }
    _ => return Column::new().into(),
  };
  Row::new()
    .id("conversation-line")
    .width(Dimension::Pct(100.0))
    .padding_vertical(lurq::app::theme::SpacingSize::Sm)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .child(
      Row::new().width(Dimension::Px(TIME_WIDTH)).flex_shrink(0.0).child(
        Text::new(&display::clock(item.at))
          .variant(TypographyStyle::Mono)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      ),
    )
    .child(
      Row::new()
        .size(Dimension::Px(GLYPH_SIZE), Dimension::Px(GLYPH_SIZE))
        .flex_shrink(0.0)
        .background(PaletteColor::SurfaceRaised)
        .rounded(lurq::app::theme::RadiusSize::Sm)
        .align_items(Alignment::Center)
        .justify(lurq::layout::layout_kind::Justify::Center)
        .child(icon(glyph, IconSize::Entry, PaletteColor::TextSecondary)),
    )
    .child(
      Text::new(&title)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .min_width(Dimension::Px(0.0))
        .flex(1.0),
    )
    .child(
      Text::new(&meta)
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .flex_shrink(0.0),
    )
    .into()
}

fn author_name(author: &Author) -> &str {
  match author {
    Author::Agent(agent) | Author::YouTo(agent) => agent,
    Author::You => "",
  }
}

/// The first line `text` shows, its Markdown rendered.
fn first_line(text: &str) -> String {
  plain_text(text).lines().next().unwrap_or_default().to_owned()
}
