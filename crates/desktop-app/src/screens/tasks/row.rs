//! One row of the task table (`Task list row` `XEQuo`): the task's key, its title with its first two tags
//! and how many more, its pipeline and priority, its workflow status over its run's status, and what it
//! needs or where it stands. A press opens the task on its Details. The task a Create just added is tinted and
//! carries the New badge (`Row · KON-37` in `t3oAOi`). Selection is not built yet (D4), so the row has no
//! checkbox.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ProjectId,
  project::TaskKey,
  task::{
    estimate::{EstimateSource, StoryEstimate},
    organization::WorkflowStatus,
  },
};
use orchester_workspace::tasks::list::{PipelineLabel, TaskRow};

use super::{readiness, table};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskView},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    chip::{BadgeTone, TagChip, badge},
    icon::{Icon, icon},
    message::message_text,
    status::{Status, StatusChip, priority_chip},
    table::TableRow,
    tooltip::Truncated,
  },
};

/// The tags a row names; the rest are counted (`More` `H6X6Gv`).
const SHOWN_TAGS: usize = 2;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RowProps {
  pub project: ProjectId,
  pub row: TaskRow,
  /// The task a Create just added.
  pub highlighted: bool,
}

impl DevtoolsInspectable for RowProps {}

pub(super) struct TaskListRow;

impl Component for TaskListRow {
  type Props = RowProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let row = &props.row;
    let key = row.key.clone();
    let navigation = Navigation::of(ctx);
    let task: TaskKey = key.parse().expect("a task's key reads back as a task key");
    // A task kept Not shared has an address of its own: a shared task may have its key (`R4elt`).
    let view = TaskView::default();
    let page = if row.not_shared {
      ProjectPage::KeptTask(task, view)
    } else {
      ProjectPage::Task(task, view)
    };
    let open = Route::Project(props.project, page);
    let pipeline = pipeline_label(ctx, &row.pipeline);
    let line = TableRow::new(format!("task-row-{key}"))
      .cell(
        table::KEY,
        Text::new(&key)
          .id(format!("task-key-{key}"))
          .variant(ExtraTypography::MonoKey.role())
          .nowrap(),
      )
      .cell(table::TASK, title_cell(ctx, row, props.highlighted))
      .cell(
        table::PIPELINE,
        Truncated::new(format!("task-pipeline-{key}"), pipeline, TypographyStyle::Description).render(ctx),
      )
      .cell(
        table::PRIORITY,
        priority_chip(ctx, &format!("task-priority-{key}"), row.organization.priority()),
      )
      .cell(table::WORKFLOW, workflow_cell(ctx, row))
      .cell(table::READINESS, readiness_cell(ctx, row))
      .opens(navigation.to(open));
    if props.highlighted { line.highlighted() } else { line }
  }
}

/// The title, cut to its room, then *was KON-12* for a task that joined with another key (`Xr4Wb`), its story
/// points (ORC-49, no design frame yet), the first tags, how many more there are, and the New badge.
fn title_cell(ctx: &mut Ctx, row: &TaskRow, highlighted: bool) -> Row {
  let key = &row.key;
  let points = row
    .estimate
    .as_ref()
    .map(|estimate| TagChip::new(format!("task-points-{key}"), points_chip(ctx, estimate)).mono());
  let tags = row
    .tags
    .iter()
    .take(SHOWN_TAGS)
    .enumerate()
    .map(|(index, name)| TagChip::new(format!("task-tag-{key}-{index}"), name.clone()));
  let more = row.tags.len().saturating_sub(SHOWN_TAGS);
  let more = (more > 0).then(|| {
    let count = ctx.t_args("tasks.moreTags", [("count", more.to_string())]);
    TagChip::new(format!("task-more-tags-{key}"), count)
  });
  let new = highlighted.then(|| badge(&format!("task-new-{key}"), BadgeTone::Info, &ctx.t("tasks.newBadge")));
  // As wide as its cell at most, so the title gives up room before the tags do.
  Row::new()
    .id(format!("task-title-{key}"))
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Truncated::new(
        format!("task-title-text-{key}"),
        row.title.clone(),
        ExtraTypography::Strong.role(),
      )
      .render(ctx),
    )
    .with_children(row.former_key.as_ref().map(|former| {
      Text::new(&ctx.t_args("tasks.formerKey", [("key", &**former)]))
        .id(format!("task-former-key-{key}"))
        .variant(ExtraTypography::MonoKey.role())
        .color(PaletteColor::TextMuted)
        .nowrap()
        .flex_shrink(0.0)
    }))
    .with_children(points)
    .with_children(tags)
    .with_children(more)
    .with_children(new)
}

/// A task's story points as a chip says them: a person's as they are (`5 pt`), Orchester's estimate marked as one
/// (`~5 pt`).
fn points_chip(ctx: &Ctx, estimate: &StoryEstimate) -> Arc<str> {
  let key = match estimate.source() {
    EstimateSource::Person => "tasks.points.person",
    EstimateSource::Automation => "tasks.points.estimated",
  };
  ctx.t_args(key, [("count", estimate.points().get().to_string())])
}

/// The workflow status, with the Blocked flag beside it, over the run's status (`Workflow cell` `x5bPKe`).
fn workflow_cell(ctx: &mut Ctx, row: &TaskRow) -> Column {
  let key = &row.key;
  let status = row.organization.status();
  let blocked = readiness::blocked(row).then(|| {
    Row::new()
      .id(format!("task-blocked-{key}"))
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .child(icon(Icon::Ban, IconSize::Sm, PaletteColor::TextSecondary))
      .child(
        Text::new(&ctx.t("tasks.blocked"))
          .variant(ExtraTypography::NoteStrong.role())
          .color(PaletteColor::TextSecondary)
          .nowrap(),
      )
  });
  // A task whose latest run cannot be read has no phase: its run reads Unreadable.
  let run = row.phase.map_or(Status::Unreadable, Status::of_phase);
  let mut run = StatusChip::new(format!("task-run-{key}"), run).compact();
  // A run at work or one that failed says at which step (`Running · 02 Plan`).
  if let Some(step) = row.run_step() {
    let number = format!("{:02}", step.number);
    run = run.detail(ctx.t_args("tasks.runStep", [("number", number.as_str()), ("step", &*step.name)]));
  }
  Column::new()
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(&ctx.t(workflow_key(status)))
            .id(format!("task-workflow-{key}"))
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextPrimary)
            .nowrap(),
        )
        .with_children(blocked),
    )
    .child(run.render(ctx))
}

/// What the task needs, or where it stands, after its icon, both in the tone it calls for.
fn readiness_cell(ctx: &mut Ctx, row: &TaskRow) -> Row {
  let key = &row.key;
  let (glyph, color) = readiness::mark(row);
  let text = message_text(ctx, &row.detail);
  // As wide as its cell at most, so a long text is cut with an ellipsis.
  Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color.clone()).id(format!("task-readiness-icon-{key}")))
    .child(
      Truncated::new(format!("task-readiness-{key}"), text, TypographyStyle::Description)
        .color(color)
        .render(ctx),
    )
}

/// The pipeline a task runs: a published one by its name and version as this list words it (`Feature
/// delivery v3`), otherwise in the workspace's words.
fn pipeline_label(ctx: &Ctx, pipeline: &PipelineLabel) -> Arc<str> {
  match pipeline {
    PipelineLabel::Published { name, version } => ctx.t_args(
      "tasks.pipelineVersion",
      [("name", name.to_string()), ("version", version.get().to_string())],
    ),
    other => message_text(ctx, &other.message()),
  }
}

/// The word of a workflow status, as the UI kit's labels say it.
const fn workflow_key(status: WorkflowStatus) -> &'static str {
  match status {
    WorkflowStatus::Backlog => "ui.workflow.backlog",
    WorkflowStatus::Todo => "ui.workflow.todo",
    WorkflowStatus::InProgress => "ui.workflow.inProgress",
    WorkflowStatus::InReview => "ui.workflow.inReview",
    WorkflowStatus::Done => "ui.workflow.done",
    WorkflowStatus::Canceled => "ui.workflow.canceled",
  }
}
