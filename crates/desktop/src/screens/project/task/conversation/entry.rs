//! One message of the conversation (`J5MIT`): who wrote it and when, what it says, and where it
//! stands — a person's queued, delivered or acknowledged, an agent's reply or question. The question
//! the run waits for an answer to stands out on the raised fill with *Reply*, which puts the focus
//! in the composer; the entries are outside the form, so pointer-only.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use crate::{
  display,
  tasks::conversation::{Author, Entry, EntryState, Queue},
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::pressable::{pressable, surface},
};

/// `focus`: set to put the focus in the composer, for *Reply*.
pub(super) fn entry(ctx: &Ctx, entry: &Entry, focus: &Signal<bool>) -> Element {
  let time = display::clock(entry.at);
  let author = match &entry.author {
    Author::You => ctx.t_args("conversation.author.you", [("time", time.as_str())]),
    Author::YouTo(agent) => ctx.t_args(
      "conversation.author.youTo",
      [("agent", &**agent), ("time", time.as_str())],
    ),
    Author::Agent(agent) => ctx.t_args(
      "conversation.author.agent",
      [("agent", &**agent), ("time", time.as_str())],
    ),
  };
  let open = entry.state == EntryState::Question { open: true };
  let state = Text::new(&state_line(ctx, &entry.state)).variant(ExtraTypography::ContextNote.role());
  let footer: Element = if open {
    Row::new()
      .width(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .child(state)
      .child(Spacer::new().flex(1.0))
      .child(reply(ctx, focus))
      .into()
  } else {
    state.into()
  };
  let message = Column::new()
    .id(format!("conversation-message-{}", entry.id))
    .describe("state", format!("{:?}", entry.state))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(&author).variant(ExtraTypography::Footnote.role()).nowrap())
    .child(
      Text::new(&entry.body)
        .variant(ExtraTypography::AgentText.role())
        .width(Dimension::Pct(100.0))
        .selectable(true),
    )
    .child(footer);
  if open {
    message
      .padding(ExtraSpacing::Stack.role())
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(ExtraRadius::Notice.role())
      .into()
  } else {
    message
      .padding_vertical(ExtraSpacing::Stack.role())
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .into()
  }
}

/// Where the message stands: "Queued · waiting for the next checkpoint", "Delivered to Builder".
fn state_line(ctx: &Ctx, state: &EntryState) -> Arc<str> {
  match state {
    EntryState::Queued(Queue::NextCheckpoint) => ctx.t("conversation.delivery.queued"),
    EntryState::Queued(Queue::RunStarts) => ctx.t("conversation.delivery.queuedRunStart"),
    EntryState::Queued(Queue::RunContinues) => ctx.t("conversation.delivery.queuedRunContinue"),
    EntryState::Queued(Queue::RunEnded) => ctx.t("conversation.delivery.notDelivered"),
    EntryState::Queued(Queue::WorktreeRemoved) => ctx.t("conversation.delivery.notDeliveredWorktree"),
    EntryState::Delivered { agent } => ctx.t_args("conversation.delivery.delivered", [("agent", &**agent)]),
    EntryState::Acknowledged { agent } => ctx.t_args("conversation.delivery.acknowledged", [("agent", &**agent)]),
    EntryState::Reply { step } => ctx.t_args("conversation.delivery.reply", [("step", &**step)]),
    EntryState::Question { open: true } => ctx.t("conversation.delivery.question"),
    EntryState::Question { open: false } => ctx.t("conversation.delivery.answered"),
  }
}

/// "Reply →": the composer takes the focus.
fn reply(ctx: &Ctx, focus: &Signal<bool>) -> Element {
  let focus = focus.clone();
  pressable(surface().id("conversation-reply"), move || focus.set(true))
    .child(
      Text::new(&ctx.t("conversation.reply"))
        .variant(TypographyStyle::Link)
        .nowrap(),
    )
    .into()
}
