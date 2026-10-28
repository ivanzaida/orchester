//! A run whose conflicts with its base went to its Integrate step. While the step works, the files that conflicted
//! (`Conflicting files` `u9QUiU` in `RRk9E`); once it could not resolve them, *Integrate couldn't resolve the conflicts
//! with main* (`hKct7`): why, the stages, the files, what Integrate did (`Verification` `lGr5x`) and the integrator's
//! last words (`Integrator's words` `zeYmm`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  integration::resolve::{FileState, Resolution},
  merge_queue::QueueEvent,
};
use orchester_workspace::display::{clock, short_revision};

use super::{
  super::ready::changed::{note, section},
  View,
  card::base,
  rows::{Look, Stage, conflict_row},
};
use crate::{
  theme::{DeliveryText, ExtraSpacing, ExtraTypography, TaskText},
  ui::{
    card::{CardSurface, card},
    chat::links,
    icon::Icon,
    markdown::MarkdownView,
    notice::{Notice, NoticeTone},
  },
};

/// *Integrate couldn't resolve the conflicts with main* and what goes with it.
pub(super) fn failed(ctx: &mut Ctx, view: &View<'_>, files: &[String]) -> Vec<Element> {
  let base = base(view.run);
  let integrator = view.integrator(ctx);
  let synced = view.queue.delivery.as_ref().and_then(|delivery| {
    delivery.events.iter().rev().find_map(|recorded| match &recorded.event {
      QueueEvent::Synced { base, .. } => Some(short_revision(base.as_str()).to_owned()),
      _ => None,
    })
  });
  let body = ctx.t_args(
    "delivery.task.failed.body",
    [
      ("base", base.to_owned()),
      ("revision", synced.unwrap_or_default()),
      ("count", files.len().to_string()),
      ("integrator", integrator.clone()),
    ],
  );
  let notice = Notice::new(
    "task-notice",
    NoticeTone::Warning,
    ctx.t_args("delivery.task.failed.title", [("base", base)]),
  )
  .icon(Icon::Hand)
  .body(body);
  let did = section(
    ctx,
    ("task-delivery-integrate-did", "delivery.task.failed.did"),
    None,
    checks_card(ctx, "task-delivery-integrate-checks", view.delivered(ctx)),
  );
  let words = integrator_words(ctx, view, &integrator);
  vec![
    notice.into(),
    view.stages(ctx, Stage::Waiting),
    conflicting(ctx, files, None),
    Row::new()
      .id("task-delivery-integrate-columns")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .align_items(Alignment::Start)
      .child(Column::new().flex(1.0).child(did))
      .with_children(words.map(|words| Column::new().flex(1.0).child(words)))
      .into(),
  ]
}

/// *Conflicting files* with their count; with the merge waiting in the run's worktree (`resolution`), how each stands.
pub(super) fn conflicting(ctx: &Ctx, files: &[String], resolution: Option<&Resolution>) -> Element {
  let rows = files.iter().enumerate().map(|(index, path)| {
    let state = resolution.and_then(|resolution| resolution.files.iter().find(|file| file.path == *path));
    let (look, said) = match state.map(|file| file.state) {
      Some(FileState::Resolved | FileState::ResolvedWithMarkers) => (Look::DONE, ctx.t("delivery.task.file.resolved")),
      Some(FileState::Undecided(_)) => (Look::CONFLICTED, ctx.t("delivery.task.file.undecided")),
      Some(FileState::Markers) => (Look::CONFLICTED, ctx.t("delivery.task.file.markers")),
      None => (Look::CONFLICTED, ctx.t("delivery.task.file.notResolved")),
    };
    let look = Look {
      color: if look.glyph == Icon::GitCompareArrows {
        PaletteColor::Warning
      } else {
        look.color
      },
      ..look
    };
    conflict_row(&format!("task-delivery-conflict-{index}"), look, path, &said)
  });
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("delivery.task.file.title")).variant(TaskText::SectionTitle.role()))
    .child(Text::new(&files.len().to_string()).variant(DeliveryText::Count.role()))
    .child(Spacer::new().flex(1.0));
  Column::new()
    .id("task-delivery-conflicts")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(head)
    .child(
      card("task-delivery-conflict-files", CardSurface::Outline)
        .padding(Padding::symmetric(ExtraSpacing::Pair.role(), ExtraSpacing::Hair.role()))
        .with_children(rows),
    )
    .into()
}

/// A card of verification lines, or a note when there are none.
pub(super) fn checks_card(ctx: &Ctx, id: &str, rows: Vec<Element>) -> Element {
  if rows.is_empty() {
    return note(&ctx.t("delivery.task.failed.nothing"));
  }
  card(id, CardSurface::Outline)
    .padding(Padding::symmetric(
      ExtraSpacing::Loose.role(),
      ExtraSpacing::Snug.role(),
    ))
    .with_children(rows)
    .into()
}

/// *In Builder's words*: what the Integrate step's agent last reported, when it did, read as Markdown.
fn integrator_words(ctx: &mut Ctx, view: &View<'_>, integrator: &str) -> Option<Element> {
  let step = view.queue.delivery.as_ref().and_then(|delivery| {
    delivery.events.iter().rev().find_map(|recorded| match &recorded.event {
      QueueEvent::IntegrateStarted { step, .. } => Some(step.clone()),
      _ => None,
    })
  })?;
  let executed = view.run.steps.iter().find(|candidate| candidate.key == step)?;
  let execution = executed.execution?;
  let said = view.run.reported(execution)?;
  let at = view
    .run
    .invocations
    .iter()
    .rev()
    .find(|invocation| invocation.execution == execution)
    .map(|invocation| clock(invocation.started_at));
  let figure = at.map(|at| ctx.t_args("delivery.task.failed.wordsAt", [("time", at)]));
  let said = MarkdownView::new("task-delivery-integrator-words-text", said.trim(), links(ctx))
    .style(ExtraTypography::Prose.role(), PaletteColor::TextSecondary)
    .render(ctx);
  let quote = card("task-delivery-integrator-words", CardSurface::Outline)
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .child(said);
  let title = ctx.t_args("delivery.task.failed.words", [("integrator", integrator)]);
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&title).variant(TaskText::SectionTitle.role()))
    .child(Spacer::new().flex(1.0))
    .with_children(figure.map(|figure| {
      Text::new(&figure)
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextMuted)
    }));
  Some(
    Column::new()
      .id("task-delivery-words")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(head)
      .child(quote)
      .into(),
  )
}
