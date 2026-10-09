//! What the chat shows instead of the run's timeline (`B67XdS`): in its middle an 18 glyph, a title in
//! 13.5/600 and what it means in 12.5/1.5 `text_secondary` at most 520 wide, 8 apart, with actions 6 under
//! them. Loading (`r5vGK`): the turning loader and *Loading activity for Run 128…*. Couldn't load (`YGnwp`):
//! `circle-alert` in `danger`, why, *Open runner log* and *Try again*. Queued (`r4KFPL`): the run's steps and
//! that nothing ran yet, or only that the task is queued while its run is not created yet. A task without a run says
//! so, or lists the messages that wait for its run; a run that cannot be read says nothing it did can be shown.

use std::path::Path;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::run::{RunStatus, TaskPhase};
use orchester_workspace::{
  messages::Message,
  tasks::{
    conversation::Conversation,
    detail::{RunView, TaskDetail},
  },
};

use super::timeline::{person_message, step_label};
use crate::{
  services::Services,
  theme::{ConversationText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
  },
};

/// The widest a state's explanation runs (`a49iH`).
const TEXT_WIDTH: f32 = 520.0;

/// `glyph` in `color`, `title`, `text` and `actions`, in the middle of the chat; `id` names it.
fn state(
  id: &str,
  (glyph, color): (Icon, PaletteColor),
  (title, text): (&str, Option<&str>),
  actions: Vec<Element>,
) -> Element {
  let actions = (!actions.is_empty()).then(|| {
    Row::new()
      .padding_top(ExtraSpacing::Snug.role())
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .with_children(actions)
  });
  Column::new()
    .id(id.to_owned())
    .describe("state", id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Section, ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(glyph, IconSize::Xl, color))
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(ConversationText::StateTitle.role())
        .text_align(TextAlign::Center),
    )
    .with_children(text.map(|text| {
      Text::new(text)
        .id(format!("{id}-text"))
        .variant(ExtraTypography::Lead.role())
        .text_align(TextAlign::Center)
        .max_width(Dimension::Px(TEXT_WIDTH))
    }))
    .with_children(actions)
    .into()
}

fn number(run: &RunView) -> String {
  run.run.number().get().to_string()
}

/// The transcript of `run` is being read (`r5vGK`).
pub(super) fn loading(ctx: &Ctx, run: &RunView) -> Element {
  let title = ctx.t_args("conversation.loading", [("run", number(run).as_str())]);
  state(
    "conversation-loading",
    (Icon::LoaderCircle, PaletteColor::TextMuted),
    (&title, None),
    Vec::new(),
  )
}

/// The transcript of `run` could not be read, and why (`YGnwp`): the run itself goes on. *Try again* bumps
/// `retry`; *Open runner log* opens the log of the runner of `repo_root`.
pub(super) fn failed(
  ctx: &mut Ctx,
  run: &RunView,
  error: &Message,
  (repo_root, retry): (&Path, &Signal<u64>),
) -> Element {
  tracing::debug!(?error, "the conversation could not be read");
  let title = ctx.t_args("conversation.failed.title", [("run", number(run).as_str())]);
  let text = ctx.t_args("conversation.failed.text", [("reason", &*message_text(ctx, error))]);
  let runners = Services::of(ctx).runners;
  let root = repo_root.to_path_buf();
  let retry = retry.clone();
  let actions = vec![
    ActionButton::new(
      "conversation-open-log",
      ButtonKind::Secondary,
      ctx.t("conversation.failed.openLog"),
    )
    .icon(Icon::FileText)
    .on_click(move || runners.open_log_of(&root))
    .into(),
    ActionButton::new("conversation-retry", ButtonKind::Primary, ctx.t("common.tryAgain"))
      .icon(Icon::RotateCcw)
      .on_click(move || retry.update(|attempt| *attempt += 1))
      .into(),
  ];
  state(
    "conversation-failed",
    (Icon::CircleAlert, PaletteColor::Danger),
    (&title, Some(&text)),
    actions,
  )
}

/// `run` has shown nothing yet: queued with its steps (`r4KFPL`), starting, or it ended without a report.
pub(super) fn nothing_yet(ctx: &Ctx, run: &RunView) -> Element {
  let title = ctx.t("conversation.empty.title");
  let text = match run.run.status() {
    RunStatus::Pending => {
      let steps = run
        .steps
        .iter()
        .map(|step| step_label(ctx, step.number, &step.name).to_string())
        .collect::<Vec<_>>()
        .join(&ctx.t("conversation.separator"));
      ctx.t_args(
        "conversation.empty.queued",
        [("steps", steps.as_str()), ("run", number(run).as_str())],
      )
    }
    status if status.is_terminal() => ctx.t("conversation.empty.none"),
    _ => ctx.t("conversation.empty.starting"),
  };
  state(
    "conversation-empty",
    (Icon::MessageSquareDashed, PaletteColor::TextMuted),
    (&title, Some(&text)),
    Vec::new(),
  )
}

/// A task without a run that can be shown: its latest run cannot be read, or it has none yet, with the
/// messages that wait for its run to start.
pub(super) fn without_run(ctx: &mut Ctx, detail: &TaskDetail, conversation: &Conversation) -> Element {
  if let Some(unreadable) = &detail.latest_unreadable {
    let title = message_text(ctx, &unreadable.title());
    let text = ctx.t("conversation.unreadable");
    return state(
      "conversation-unreadable",
      (Icon::CircleAlert, PaletteColor::Danger),
      (&title, Some(&text)),
      Vec::new(),
    );
  }
  if conversation.entries.is_empty() && detail.phase() == TaskPhase::Queued {
    let title = ctx.t("conversation.empty.title");
    let text = ctx.t("conversation.empty.queuedTask");
    return state(
      "conversation-empty",
      (Icon::MessageSquareDashed, PaletteColor::TextMuted),
      (&title, Some(&text)),
      Vec::new(),
    );
  }
  if conversation.entries.is_empty() {
    let title = ctx.t("conversation.noRun.title");
    let text = ctx.t("conversation.noRun.text");
    return state(
      "conversation-no-run",
      (Icon::Clock, PaletteColor::TextMuted),
      (&title, Some(&text)),
      Vec::new(),
    );
  }
  let messages: Vec<Element> = conversation
    .entries
    .iter()
    .map(|entry| person_message(ctx, entry))
    .collect();
  Column::new()
    .id("conversation-waiting")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Section, ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Md)
    .with_children(messages)
    .into()
}
