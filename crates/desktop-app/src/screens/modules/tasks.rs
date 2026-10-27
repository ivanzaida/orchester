//! *Tasks · 12* on a module's Summary (`Tasks` `k28wU` in `02 · Module · Summary`): the module's tasks as one-line rows
//! in the Tasks list's style, grouped by workflow status (In progress, In review, Todo, Backlog, then Done and
//! Canceled, folded until opened), each group's line with its chevron, icon, word and count folding it. A row is 40
//! high, 8 at its sides, its parts 12 apart: the status icon, the key (mono 11.5 `text_muted`, 52 wide), the title in
//! 13/500 and the task's age (mono 11 `text_muted`, 30 wide), and opens the task. A module without tasks says how to
//! put some in.

use std::collections::BTreeSet;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  task::{Task, organization::WorkflowStatus},
  time::Timestamp,
};

use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskView},
  screens::{task::summary::section_head, tasks::age},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, TasksSpacing},
  ui::{
    icon::{Icon, icon},
    pressable::{Press, pressable},
    status::{workflow_key, workflow_look},
    tooltip::Truncated,
  },
};

/// A group's line and a task's row (`Group · In progress` `hGpQ7`, `KON-12` `wpVcf`), and a row's key and age.
const GROUP_HEIGHT: f32 = 34.0;
const ROW_HEIGHT: f32 = 40.0;
const KEY_WIDTH: f32 = 52.0;
const AGE_WIDTH: f32 = 30.0;

/// The groups in their order.
const ORDER: [WorkflowStatus; 6] = [
  WorkflowStatus::InProgress,
  WorkflowStatus::InReview,
  WorkflowStatus::Todo,
  WorkflowStatus::Backlog,
  WorkflowStatus::Done,
  WorkflowStatus::Canceled,
];

/// Which groups the person opened or folded, each against how it starts: Done and Canceled folded, the rest open.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Toggled(pub BTreeSet<&'static str>);

impl lurq::app::component::DevtoolsInspectable for Toggled {}

impl Toggled {
  fn folded(&self, status: WorkflowStatus) -> bool {
    let starts_folded = matches!(status, WorkflowStatus::Done | WorkflowStatus::Canceled);
    starts_folded != self.0.contains(status_name(status))
  }
}

pub fn section(ctx: &mut Ctx, project: ProjectId, tasks: &[Task], toggled: &Signal<Toggled>) -> Element {
  let head = section_head(ctx, "module-tasks-head", "modules.page.tasks", Some(tasks.len()));
  let body: Element = if tasks.is_empty() {
    Text::new(&ctx.t("modules.page.noTasks"))
      .id("module-no-tasks")
      .variant(TypographyStyle::Description)
      .width(Dimension::Pct(100.0))
      .into()
  } else {
    list(ctx, project, tasks, toggled)
  };
  Column::new()
    .id("module-tasks-section")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(head)
    .child(body)
    .into()
}

fn list(ctx: &mut Ctx, project: ProjectId, tasks: &[Task], toggled: &Signal<Toggled>) -> Element {
  let shown = toggled.get();
  let now = Timestamp::now();
  let mut lines = Vec::new();
  for status in ORDER {
    let members: Vec<&Task> = tasks
      .iter()
      .filter(|task| task.organization().status() == status)
      .collect();
    if members.is_empty() {
      continue;
    }
    let folded = shown.folded(status);
    lines.push(group_line(ctx, (status, members.len(), folded), toggled));
    if !folded {
      lines.extend(members.into_iter().map(|task| row(ctx, project, task, now)));
    }
  }
  Column::new()
    .id("module-tasks")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .with_children(lines)
    .into()
}

const fn status_name(status: WorkflowStatus) -> &'static str {
  match status {
    WorkflowStatus::Backlog => "backlog",
    WorkflowStatus::Todo => "todo",
    WorkflowStatus::InProgress => "in_progress",
    WorkflowStatus::InReview => "in_review",
    WorkflowStatus::Done => "done",
    WorkflowStatus::Canceled => "canceled",
  }
}

/// A group's line, which folds the group away or opens it.
fn group_line(ctx: &Ctx, (status, count, folded): (WorkflowStatus, usize, bool), toggled: &Signal<Toggled>) -> Element {
  let name = status_name(status);
  let id = format!("module-group-{name}");
  let (glyph, tone) = workflow_look(status);
  let label = ctx.t(workflow_key(status));
  let toggled = toggled.clone();
  pressable(move || {
    toggled.update(|toggled| {
      if !toggled.0.remove(name) {
        toggled.0.insert(name);
      }
    });
  })
  .id(id.clone())
  .width(Dimension::Pct(100.0))
  .height(Dimension::Px(GROUP_HEIGHT))
  .padding(
    Padding::new()
      .top(TasksSpacing::GroupTop.role())
      .left(SpacingSize::Sm)
      .right(SpacingSize::Sm),
  )
  .spacing(SpacingSize::Sm)
  .align_items(Alignment::Center)
  .describe(
    "a11y_name",
    ctx.t_args("modules.summary.toggleGroup", [("group", &*label)]),
  )
  .describe("state", if folded { "folded" } else { "open" })
  .child(icon(
    if folded { Icon::ChevronRight } else { Icon::ChevronDown },
    IconSize::Sm,
    PaletteColor::TextMuted,
  ))
  .child(icon(glyph, IconSize::Base, tone))
  .child(
    Text::new(&label)
      .id(format!("{id}-label"))
      .variant(TypographyStyle::Label)
      .color(PaletteColor::TextSecondary)
      .nowrap(),
  )
  .child(
    Text::new(&count.to_string())
      .id(format!("{id}-count"))
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted)
      .nowrap(),
  )
  .into()
}

/// A task's row, which opens the task.
fn row(ctx: &mut Ctx, project: ProjectId, task: &Task, now: Timestamp) -> Element {
  let key = task.key().to_string();
  let (glyph, tone) = workflow_look(task.organization().status());
  let line = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Md, tone))
    .child(
      Row::new().width(Dimension::Px(KEY_WIDTH)).flex_shrink(0.0).child(
        Text::new(&key)
          .id(format!("module-task-key-{key}"))
          .variant(ExtraTypography::MonoKey.role())
          .nowrap(),
      ),
    )
    .child(
      Row::new().flex(1.0).min_width(Dimension::Px(0.0)).child(
        Truncated::new(
          format!("module-task-title-{key}"),
          task.details().title.as_str().to_owned(),
          ExtraTypography::Strong.role(),
        )
        .render(ctx),
      ),
    )
    .child(
      Row::new()
        .width(Dimension::Px(AGE_WIDTH))
        .flex_shrink(0.0)
        .justify(Justify::End)
        .child(
          Text::new(&age(ctx, now, task.created_at()))
            .variant(ExtraTypography::MonoNote.role())
            .nowrap(),
        ),
    );
  let navigation = Navigation::of(ctx);
  let page = ProjectPage::Task(task.key().clone(), TaskView::default());
  Press::Open(navigation.to(Route::Project(project, page)))
    .button()
    .id(format!("module-task-{key}"))
    .width(Dimension::Pct(100.0))
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(line)
    .into()
}
