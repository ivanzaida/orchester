//! An event of a task's run in the Orchestrator's conversation (ORC-755; `Parts · Lines` e `c9YRv` and `04 · Leading
//! two modules` `OKtX2` in `Orchestrator watch · redesign`): one `border` line 34 high, radius 6, 12 at its sides and
//! its parts 10 apart: a 13 icon in its tone, what happened (12.5 `text_secondary`), the task and the run in Geist Mono
//! 11.5 `text_muted`, the task's module in 11.5 `text_muted` filling the line (none without a module), the time in
//! Geist Mono 11 `text_muted`, and the one link that acts on it. A run without progress links to the run's
//! Conversation (*Run ↗*), a merge conflict to the task's Summary, where it is resolved (*Resolve ↗*); a task the run
//! waits on names that task and its run, and links to its Changes once merged (*Changes ↗*), else to its Summary
//! (*Open ↗*). A run without progress that moved again, ended or waits now reads as past (ORC-894): without the
//! outline, its icon muted, saying what became of the run (*No progress for 20 min · moving again 20:50*).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  orchestrator::{AgentWait, TaskEvent, TaskEventKind},
  project::TaskKey,
  time::Timestamp,
};
use orchester_workspace::{
  display,
  orchestrator::chat::{EventFacts, StallSettled},
};

use super::line::{LineLink, link_element};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  theme::{ExtraSpacing, ExtraTypography, IconSize, OrchestratorText},
  ui::{
    icon::{Icon, icon},
    pressable::Press,
  },
};

/// The line's height (`K3wD5h`).
const HEIGHT: f32 = 34.0;

/// The line of `event` of `wait`'s run, kept at `at`, with what it names beside the wait (`facts`, empty while they
/// are not read); in project `project`; `id` names it. A stall that no longer stands (its facts say what became of the
/// run, ORC-894) is past: it keeps its place and its parts, without the outline, its icon muted.
pub(super) fn line(
  ctx: &mut Ctx,
  id: &str,
  (wait, event, at): (&AgentWait, TaskEvent, Option<Timestamp>),
  (facts, project): (Option<&EventFacts>, ProjectId),
) -> Element {
  let empty = EventFacts::default();
  let facts = facts.unwrap_or(&empty);
  let settled = facts.settled.is_some();
  let (glyph, color) = look(event.kind);
  let color = if settled { PaletteColor::TextMuted } else { color };
  let what = what(ctx, event.kind, facts);
  let target = target(ctx, wait, facts);
  let link = link(ctx, event.kind, (wait, facts), project);
  let module = facts.module.as_ref().map(|module| {
    Element::from(
      Text::new(module)
        .id(format!("{id}-module"))
        .variant(ExtraTypography::Note.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
  });
  let fill = module.unwrap_or_else(|| Spacer::new().flex(1.0).into());
  let row = Row::new()
    .id(id.to_owned())
    .describe("outcome", name(event.kind))
    .describe("status", if settled { "settled" } else { "stands" })
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md);
  // An inside outline takes no room: the past line keeps its parts where they were.
  let row = if settled {
    row
  } else {
    row.border(Border::inside(BorderSize::Sm, PaletteColor::Border))
  };
  row
    .child(icon(glyph, IconSize::Base, color).flex_shrink(0.0))
    .child(
      Text::new(&what)
        .id(format!("{id}-title"))
        .variant(OrchestratorText::Line.role())
        .color(PaletteColor::TextSecondary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(
      Text::new(&target)
        .id(format!("{id}-target"))
        .variant(ExtraTypography::MonoKey.role())
        .nowrap()
        .flex_shrink(0.0),
    )
    .child(fill)
    .with_children(at.map(|at| {
      Text::new(&display::clock(at))
        .id(format!("{id}-time"))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap()
        .flex_shrink(0.0)
    }))
    .with_children(link.map(|link| link_element(&format!("{id}-open"), link)))
    .into()
}

/// The icon of an event of `kind` and its tone: a task the run waited on merged is good news, the rest wait for
/// someone.
const fn look(kind: TaskEventKind) -> (Icon, PaletteColor) {
  match kind {
    TaskEventKind::NoProgress { .. } => (Icon::TimerOff, PaletteColor::Warning),
    TaskEventKind::MergeConflict => (Icon::GitPullRequestArrow, PaletteColor::Warning),
    TaskEventKind::WaitedMerged { .. } => (Icon::GitMerge, PaletteColor::Success),
    TaskEventKind::WaitedClosed { .. } => (Icon::Ban, PaletteColor::Warning),
  }
}

/// What the line says happened: the merges name the branch the run merges into, when it is known; a stall that settled
/// says what became of its run.
fn what(ctx: &Ctx, kind: TaskEventKind, facts: &EventFacts) -> Arc<str> {
  let branch = facts.branch.as_deref();
  match (kind, branch) {
    (TaskEventKind::NoProgress { minutes, .. }, _) => {
      let minutes = ("minutes", minutes.to_string());
      match facts.settled {
        None => ctx.t_args("orchestrator.event.noProgress", [minutes]),
        Some(StallSettled::Moving(at)) => ctx.t_args(
          "orchestrator.event.noProgressMoving",
          [minutes, ("time", display::clock(at))],
        ),
        Some(StallSettled::Waits) => ctx.t_args("orchestrator.event.noProgressWaits", [minutes]),
        Some(StallSettled::Ended) => ctx.t_args("orchestrator.event.noProgressEnded", [minutes]),
      }
    }
    (TaskEventKind::MergeConflict, _) if facts.module.is_some() => ctx.t("orchestrator.event.conflictModule"),
    (TaskEventKind::MergeConflict, _) => ctx.t("orchestrator.event.conflict"),
    (TaskEventKind::WaitedMerged { .. }, Some(branch)) => {
      ctx.t_args("orchestrator.event.mergedInto", [("branch", branch)])
    }
    (TaskEventKind::WaitedMerged { .. }, None) => ctx.t("orchestrator.event.merged"),
    (TaskEventKind::WaitedClosed { .. }, Some(branch)) => {
      ctx.t_args("orchestrator.event.closedInto", [("branch", branch)])
    }
    (TaskEventKind::WaitedClosed { .. }, None) => ctx.t("orchestrator.event.closed"),
  }
}

/// The task and the run the line is about: the task the run waits on, once it is known, else the wait's own.
fn target(ctx: &Ctx, wait: &AgentWait, facts: &EventFacts) -> Arc<str> {
  match &facts.waited {
    Some(waited) => match waited.number {
      Some(number) => ctx.t_args(
        "orchestrator.line.run",
        [("task", waited.key.as_str()), ("run", &number.to_string())],
      ),
      None => Arc::from(waited.key.as_str()),
    },
    None => super::wait::run_target(ctx, wait),
  }
}

/// The link that acts on an event of `kind` of `wait`'s run.
fn link(
  ctx: &mut Ctx,
  kind: TaskEventKind,
  (wait, facts): (&AgentWait, &EventFacts),
  project: ProjectId,
) -> Option<LineLink> {
  let waited = facts
    .waited
    .as_ref()
    .map_or(wait.task.as_str(), |waited| waited.key.as_str());
  let (label, task, tab) = match kind {
    TaskEventKind::NoProgress { .. } => ("orchestrator.event.link.run", wait.task.as_str(), TaskTab::Conversation),
    TaskEventKind::MergeConflict => ("orchestrator.event.link.resolve", wait.task.as_str(), TaskTab::Summary),
    TaskEventKind::WaitedMerged { .. } => ("orchestrator.event.link.changes", waited, TaskTab::Changes),
    TaskEventKind::WaitedClosed { .. } => ("orchestrator.line.open", waited, TaskTab::Summary),
  };
  let key: TaskKey = task.parse().ok()?;
  let navigation = Navigation::of(ctx);
  let route = Route::Project(project, ProjectPage::Task(key, TaskView::of(tab)));
  Some(LineLink {
    label: ctx.t(label),
    press: Press::Open(navigation.to(route)),
  })
}

/// The name tools read of an event of `kind`.
const fn name(kind: TaskEventKind) -> &'static str {
  match kind {
    TaskEventKind::NoProgress { .. } => "no_progress",
    TaskEventKind::MergeConflict => "merge_conflict",
    TaskEventKind::WaitedMerged { .. } => "waited_merged",
    TaskEventKind::WaitedClosed { .. } => "waited_closed",
  }
}
