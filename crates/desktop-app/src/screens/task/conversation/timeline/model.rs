//! What the run's model chip changed (the event lines of 08f `Yzb3F`; `Y9kSm` in 08c `b3a6z`, the waiting line in 08d
//! `ff1nD`): a quiet row 36 in, a 13 glyph, what changed in 12 `text_secondary`, the model and effort it changed to in
//! Geist Mono 12 `text_primary`, then what it was, who and when in 12 `text_muted`, 10 apart. A change for this step
//! (`cpu`), for the rest of the run (`cpu`, naming the later steps it reached), of the effort alone (`gauge`); while it
//! waits for the turn that runs (`loader-circle`) it ends in *Cancel change* in 12/500 `text_secondary`; a cancelation
//! (`circle-slash`) says what the step stays on.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  resource::StepKey,
  run::{Answerer, ModelChoice, ModelEventKind, ModelScope},
  time::Timestamp,
};
use orchester_workspace::{
  display,
  tasks::{run_model::back::BackToProfile, timeline::model_events::ModelLine},
};

use super::Timeline;
use crate::{
  screens::task::conversation::model::{RunModel, choice_words, effort_word, step_name},
  theme::{ConversationSpacing, ConversationText, ExtraSpacing, ExtraTypography, IconSize, PermissionsText},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

pub(super) fn row(ctx: &mut Ctx, timeline: &Timeline, (key, line, at): (&str, &ModelLine, Timestamp)) -> Element {
  let id = format!("conversation-model-{key}");
  let time = display::clock(at);
  let (glyph, lead, value, detail) = match &line.kind {
    ModelEventKind::Changed { .. } if line.waits => (
      Icon::LoaderCircle,
      "conversation.model.event.waits",
      Some(value_words(ctx, &line.kind)),
      waits_detail(ctx, line, &time),
    ),
    ModelEventKind::Changed { scope, .. } => {
      let (glyph, lead) = match (effort_line(&line.kind), scope) {
        (true, _) => (Icon::Gauge, "conversation.model.event.effort"),
        (false, ModelScope::Run) => (Icon::Cpu, "conversation.model.event.changedRun"),
        (false, ModelScope::Step) => (Icon::Cpu, "conversation.model.event.changedStep"),
      };
      (
        glyph,
        lead,
        Some(value_words(ctx, &line.kind)),
        changed_detail(ctx, timeline, line, &time),
      )
    }
    ModelEventKind::Canceled { stays } => (
      Icon::CircleSlash,
      "conversation.model.event.canceled",
      None,
      canceled_detail(ctx, line.by, stays, &time),
    ),
  };
  let value = value.map(|value| {
    Text::new(&value)
      .id(format!("{id}-value"))
      .variant(ExtraTypography::PropertyMono.role())
      .color(PaletteColor::TextPrimary)
      .nowrap()
  });
  let cancel = cancel(ctx, timeline, &id, line);
  Row::new()
    .id(id.clone())
    .describe("kind", kind_name(line))
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(ConversationSpacing::ApprovalRow.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t(lead))
        .id(format!("{id}-text"))
        .variant(ConversationText::ApprovalLead.role())
        .nowrap(),
    )
    .with_children(value)
    .child(
      Text::new(&detail)
        .id(format!("{id}-detail"))
        .variant(ConversationText::ApprovalDetail.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(cancel)
    .into()
}

/// *05 Verify starts on its profile*: a later step started on its own profile again after a change for one step only
/// (`rotate-ccw`), its profile's model and effort in Geist Mono, then which step the change was for.
pub(super) fn back_row(ctx: &mut Ctx, timeline: &Timeline, key: &str, back: &BackToProfile) -> Element {
  let id = format!("conversation-model-{key}");
  let name = |step: &StepKey| -> Arc<str> {
    timeline
      .detail
      .run
      .as_ref()
      .map_or_else(|| step.as_str().into(), |run| step_name(ctx, run, step))
  };
  let (step, changed) = (name(&back.step), name(&back.changed));
  Row::new()
    .id(id.clone())
    .describe("kind", "back-to-profile")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(ConversationSpacing::ApprovalRow.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::RotateCcw, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t_args("conversation.model.event.backToProfile", [("step", &*step)]))
        .id(format!("{id}-text"))
        .variant(ConversationText::ApprovalLead.role())
        .nowrap(),
    )
    .child(
      Text::new(&choice_words(ctx, &back.profile))
        .id(format!("{id}-value"))
        .variant(ExtraTypography::PropertyMono.role())
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .child(
      Text::new(&ctx.t_args("conversation.model.event.detail.backToProfile", [("step", &*changed)]))
        .id(format!("{id}-detail"))
        .variant(ConversationText::ApprovalDetail.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// Whether the line is *Effort changed for this step*: a change of the effort alone, for this step.
fn effort_line(kind: &ModelEventKind) -> bool {
  kind.changes_effort_only()
    && matches!(
      kind,
      ModelEventKind::Changed {
        scope: ModelScope::Step,
        ..
      }
    )
}

/// What a change changed to: the effort alone on an effort's line, else the model and its effort.
fn value_words(ctx: &Ctx, kind: &ModelEventKind) -> Arc<str> {
  match kind {
    ModelEventKind::Changed { to, .. } if effort_line(kind) => effort_of(ctx, to),
    ModelEventKind::Changed { to, .. } => choice_words(ctx, to),
    ModelEventKind::Canceled { stays } => choice_words(ctx, stays),
  }
}

/// A choice's effort as the line says it: its word, or the CLI's default.
fn effort_of(ctx: &Ctx, choice: &ModelChoice) -> Arc<str> {
  choice.effort.as_deref().map_or_else(
    || ctx.t("conversation.model.effortDefault"),
    |effort| effort_word(ctx, effort),
  )
}

fn changed_detail(ctx: &Ctx, timeline: &Timeline, line: &ModelLine, time: &str) -> Arc<str> {
  let ModelEventKind::Changed { was, also, .. } = &line.kind else {
    return Arc::from("");
  };
  let was = if effort_line(&line.kind) {
    effort_of(ctx, was)
  } else {
    choice_words(ctx, was)
  };
  let person = line.by == Answerer::Person;
  let run = timeline.detail.run.as_ref();
  let steps: Vec<Arc<str>> = also
    .iter()
    .map(|key| run.map_or_else(|| key.as_str().into(), |run| step_name(ctx, run, key)))
    .collect();
  if steps.is_empty() {
    let key = if person {
      "conversation.model.event.detail.wasPerson"
    } else {
      "conversation.model.event.detail.wasAutomation"
    };
    return ctx.t_args(key, [("was", &*was), ("time", time)]);
  }
  let key = if person {
    "conversation.model.event.detail.alsoPerson"
  } else {
    "conversation.model.event.detail.alsoAutomation"
  };
  let steps = steps.iter().map(|name| &**name).collect::<Vec<_>>().join(", ");
  ctx.t_args(key, [("steps", steps.as_str()), ("was", &*was), ("time", time)])
}

fn waits_detail(ctx: &Ctx, line: &ModelLine, time: &str) -> Arc<str> {
  let ModelEventKind::Changed { was, scope, .. } = &line.kind else {
    return Arc::from("");
  };
  let key = match (scope, line.by) {
    (ModelScope::Step, Answerer::Person) => "conversation.model.event.detail.waitsPerson",
    (ModelScope::Step, Answerer::Automation) => "conversation.model.event.detail.waitsAutomation",
    (ModelScope::Run, Answerer::Person) => "conversation.model.event.detail.waitsRunPerson",
    (ModelScope::Run, Answerer::Automation) => "conversation.model.event.detail.waitsRunAutomation",
  };
  ctx.t_args(key, [("time", time), ("was", &*choice_words(ctx, was))])
}

fn canceled_detail(ctx: &Ctx, by: Answerer, stays: &ModelChoice, time: &str) -> Arc<str> {
  let key = match by {
    Answerer::Person => "conversation.model.event.detail.canceledPerson",
    Answerer::Automation => "conversation.model.event.detail.canceledAutomation",
  };
  ctx.t_args(key, [("time", time), ("stays", &*choice_words(ctx, stays))])
}

/// *Cancel change* on the change that waits for its turn to end, while the run goes on.
fn cancel(ctx: &mut Ctx, timeline: &Timeline, id: &str, line: &ModelLine) -> Option<Element> {
  let run = timeline
    .detail
    .run
    .as_ref()
    .filter(|run| line.waits && !run.run.status().is_terminal())?;
  let model = RunModel::of(ctx);
  let (root, run, step) = (timeline.chat_project.root.clone(), run.id(), line.step.clone());
  Some(
    pressable(move || model.cancel(&root, run, step.clone()))
      .id(format!("{id}-cancel"))
      .describe("state", "enabled")
      .child(
        Text::new(&ctx.t("conversation.model.event.cancel"))
          .variant(PermissionsText::Action.role())
          .nowrap(),
      )
      .into(),
  )
}

/// The name `describe` reports a line by, for tests and QA.
fn kind_name(line: &ModelLine) -> &'static str {
  match &line.kind {
    ModelEventKind::Changed { .. } if line.waits => "waits",
    ModelEventKind::Changed { .. } if effort_line(&line.kind) => "effort",
    ModelEventKind::Changed {
      scope: ModelScope::Run, ..
    } => "changed-run",
    ModelEventKind::Changed { .. } => "changed-step",
    ModelEventKind::Canceled { .. } => "canceled",
  }
}
