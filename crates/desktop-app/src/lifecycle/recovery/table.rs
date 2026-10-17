//! The interrupted runs' table (`JSRBO`): each run's task and number, the step it was interrupted
//! during with its agent, why, its last checkpoint, and what its worktree holds beyond it, with *View
//! changes* on the run's task page. Every value is runtime data.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, run::InterruptCause};
use orchester_workspace::{
  display,
  recovery::{InterruptedRun, WorktreeState},
};

use super::super::rows::provider_key;
use crate::{
  navigation::Navigation,
  routes::{ChangeSource, ChangedFile, ProjectPage, Route, TaskTab, TaskView},
  theme::{ExtraSpacing, ExtraTypography, IconSize, LifecycleText},
  ui::{
    icon::{Icon, icon},
    pressable::opening,
    table::{ColumnWidth, table_header},
  },
};

/// The columns after the task's (`INTERRUPTED DURING`, `REASON`, `LAST CHECKPOINT`, `WORKTREE` in
/// `JSRBO`).
const DURING: ColumnWidth = ColumnWidth::Fixed(184.0);
const REASON: ColumnWidth = ColumnWidth::Fixed(174.0);
const CHECKPOINT: ColumnWidth = ColumnWidth::Fixed(114.0);
const WORKTREE: ColumnWidth = ColumnWidth::Fixed(190.0);

/// The table of `runs`, all of project `project`.
pub fn runs(ctx: &mut Ctx, project: ProjectId, runs: &[InterruptedRun]) -> Element {
  let headings = [
    (ColumnWidth::Fill, Some(ctx.t("lifecycle.recovery.columns.task"))),
    (DURING, Some(ctx.t("lifecycle.recovery.columns.during"))),
    (REASON, Some(ctx.t("lifecycle.recovery.columns.reason"))),
    (CHECKPOINT, Some(ctx.t("lifecycle.recovery.columns.checkpoint"))),
    (WORKTREE, Some(ctx.t("lifecycle.recovery.columns.worktree"))),
  ];
  let columns: Vec<(ColumnWidth, Option<&str>)> = headings
    .iter()
    .map(|(width, heading)| (*width, heading.as_deref()))
    .collect();
  let navigation = Navigation::of(ctx);
  let last = runs.len().saturating_sub(1);
  Column::new()
    .id("recovery-runs")
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Lg)
    .clip()
    .child(table_header("recovery-runs-header", &columns, false))
    .with_children(
      runs
        .iter()
        .enumerate()
        .map(|(index, run)| row(ctx, (project, &navigation), run, index == last)),
    )
    .into()
}

fn row(ctx: &Ctx, (project, navigation): (ProjectId, &Navigation), run: &InterruptedRun, last: bool) -> Element {
  let line = Row::new()
    .id(format!("recovery-run-{}", run.run))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Md))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(task(ctx, run))
    .child(cell(DURING, during(ctx, run)))
    .child(cell(
      REASON,
      Text::new(&ctx.t(reason_key(run.cause)))
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0))
        .into(),
    ))
    .child(cell(CHECKPOINT, checkpoint(ctx, run)))
    .child(cell(WORKTREE, worktree(ctx, (project, navigation), run)));
  if last {
    line.into()
  } else {
    line
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .into()
  }
}

fn cell(width: ColumnWidth, content: Element) -> Row {
  let cell = Row::new().child(content);
  match width {
    ColumnWidth::Fixed(width) => cell.width(Dimension::Px(width)).flex_shrink(0.0),
    ColumnWidth::Fill => cell.flex(1.0).min_width(Dimension::Px(0.0)),
  }
}

/// The task's key and title over the run's number.
fn task(ctx: &Ctx, run: &InterruptedRun) -> Column {
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(&run.task_key.to_string())
            .variant(ExtraTypography::MonoKey.role())
            .nowrap()
            .flex_shrink(0.0),
        )
        .child(
          Text::new(&run.task_title)
            .variant(ExtraTypography::Strong.role())
            .nowrap()
            .text_overflow(TextOverflow::Elipsis)
            .flex_shrink(1.0),
        ),
    )
    .child(
      Text::new(&ctx.t_args("lifecycle.recovery.run", [("number", run.number.to_string())]))
        .variant(ExtraTypography::Note.role()),
    )
}

/// The step it was interrupted during, and its agent · provider · model.
fn during(ctx: &Ctx, run: &InterruptedRun) -> Element {
  let step = run.step.clone().unwrap_or_else(|| ctx.t("lifecycle.recovery.noStep"));
  let agent = match (&run.agent, run.provider, &run.model) {
    (Some(agent), Some(provider), Some(model)) => Some(ctx.t_args(
      "lifecycle.recovery.agent",
      [
        ("agent", &**agent),
        ("provider", &*ctx.t(provider_key(provider))),
        ("model", &**model),
      ],
    )),
    (Some(agent), ..) => Some(agent.clone()),
    _ => None,
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(Text::new(&step).variant(LifecycleText::Cell.role()))
    .with_children(agent.map(|agent| {
      Text::new(&agent)
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0))
    }))
    .into()
}

const fn reason_key(cause: InterruptCause) -> &'static str {
  match cause {
    InterruptCause::QuitNow => "lifecycle.recovery.reason.quitNow",
    InterruptCause::Crashed => "lifecycle.recovery.reason.crashed",
    InterruptCause::RunnerLost => "lifecycle.recovery.reason.runnerLost",
    InterruptCause::RunnerFailed => "lifecycle.recovery.reason.runnerFailed",
  }
}

/// The last checkpoint's revision and when it was saved, or that there is none yet.
fn checkpoint(ctx: &Ctx, run: &InterruptedRun) -> Element {
  let Some((revision, saved)) = &run.checkpoint else {
    return Text::new(&ctx.t("lifecycle.recovery.noCheckpoint"))
      .variant(ExtraTypography::Note.role())
      .into();
  };
  Column::new()
    .spacing(ExtraSpacing::Fine.role())
    .child(Text::new(revision).variant(TypographyStyle::Mono))
    .child(
      Text::new(&ctx.t_args("lifecycle.recovery.checkpointSaved", [("time", display::clock(*saved))]))
        .variant(ExtraTypography::Note.role()),
    )
    .into()
}

/// What the worktree holds beyond the checkpoint; uncommitted files with *View changes*.
fn worktree(ctx: &Ctx, (project, navigation): (ProjectId, &Navigation), run: &InterruptedRun) -> Element {
  let (glyph, words) = match run.worktree {
    WorktreeState::Clean => (Icon::Check, ctx.t("lifecycle.recovery.worktree.clean")),
    WorktreeState::Unknown => (Icon::CircleAlert, ctx.t("lifecycle.recovery.worktree.unknown")),
    WorktreeState::Uncommitted(files) => {
      let key = if files == 1 {
        "lifecycle.recovery.worktree.uncommitted.one"
      } else {
        "lifecycle.recovery.worktree.uncommitted.other"
      };
      (Icon::FileDiff, ctx.t_args(key, [("count", files.to_string())]))
    }
  };
  let uncommitted = matches!(run.worktree, WorktreeState::Uncommitted(_));
  let (color, role) = if uncommitted {
    (PaletteColor::TextSecondary, ExtraTypography::Prose.role())
  } else {
    (PaletteColor::TextMuted, ExtraTypography::Note.role())
  };
  let state = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Sm, color))
    .child(Text::new(&words).variant(role).flex(1.0));
  // *View changes* opens the files that are not committed, from the first of them; git names no file
  // with an empty path, and the tab would then open on its list.
  let view = run.uncommitted_file.as_deref().map(|file| {
    let view = ChangedFile::new(file, ChangeSource::Worktree)
      .map_or(TaskView::of(TaskTab::Changes), |file| TaskView::Changes(Some(file)));
    opening(navigation.to(Route::Project(project, ProjectPage::Task(run.task_key.clone(), view))))
      .id(format!("recovery-view-changes-{}", run.run))
      .cursor(CursorIcon::Pointer)
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(
        Text::new(&ctx.t("lifecycle.recovery.viewChanges"))
          .variant(LifecycleText::Link.role())
          .nowrap(),
      )
      .child(icon(Icon::ArrowRight, IconSize::Sm, PaletteColor::TextPrimary))
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(state)
    .with_children(view)
    .into()
}
