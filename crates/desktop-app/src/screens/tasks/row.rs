//! One row of the task table (`Task list row` `XEQuo`): the task's key, its title with its first two tags
//! and how many more, its pipeline and priority, its workflow status over its run's status, and what it
//! needs or where it stands. A press opens the task's Summary. The task a Create just added is tinted and
//! carries the New badge (`Row · KON-37` in `t3oAOi`). Selection is not built yet (D4), so the row has no
//! checkbox.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::Element,
};
use orchester_core::{id::ProjectId, project::TaskKey, task::organization::WorkflowStatus};
use orchester_workspace::tasks::list::TaskRow;

use super::table;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskTab, TaskView},
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
  /// Another task blocks it, or it is marked Blocked.
  pub blocked: bool,
  /// The icon of what its readiness cell says, and the tone of both (`readiness::mark`).
  pub mark: (Icon, PaletteColor),
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
    let open = Route::Project(props.project, ProjectPage::Task(task, TaskView::of(TaskTab::Summary)));
    let pipeline = message_text(ctx, &row.pipeline.message());
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
      .cell(table::WORKFLOW, workflow_cell(ctx, row, props.blocked))
      .cell(table::READINESS, readiness_cell(ctx, row, props.mark.clone()))
      .on_open(move || navigation.go(open.clone()));
    if props.highlighted { line.highlighted() } else { line }
  }
}

/// The title, cut to its room, then the first tags, how many more there are, and the New badge.
fn title_cell(ctx: &mut Ctx, row: &TaskRow, highlighted: bool) -> Row {
  let key = &row.key;
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
  Row::new()
    .id(format!("task-title-{key}"))
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
    .with_children(tags)
    .with_children(more)
    .with_children(new)
}

/// The workflow status, with the Blocked flag beside it, over the run's status (`Workflow cell` `x5bPKe`).
fn workflow_cell(ctx: &mut Ctx, row: &TaskRow, blocked: bool) -> Column {
  let key = &row.key;
  let status = row.organization.status();
  let blocked = blocked.then(|| {
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
    .child(StatusChip::new(format!("task-run-{key}"), run).compact().render(ctx))
}

/// What the task needs, or where it stands, after its icon, both in the tone it calls for.
fn readiness_cell(ctx: &mut Ctx, row: &TaskRow, (glyph, color): (Icon, PaletteColor)) -> Row {
  let key = &row.key;
  let text = message_text(ctx, &row.detail);
  Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color.clone()).id(format!("task-readiness-icon-{key}")))
    .child(
      Truncated::new(format!("task-readiness-{key}"), text, TypographyStyle::Description)
        .color(color)
        .render(ctx),
    )
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
