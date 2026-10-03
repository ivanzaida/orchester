//! Why Run is unavailable while blocking tasks are unfinished, beside the task form's run action
//! (`qlttu` in `ZLhiM`): the unfinished tasks, the linked tasks that no longer block, and how the
//! manual Blocked flag is cleared.

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::TaskId,
  task::organization::{TaskOrganization, WorkflowStatus},
};

use crate::{
  tasks::{data::ProjectTasks, organization::workflow_copy},
  ui::organization_notice::organization_notice,
};

/// A linked task: its key, title and workflow status.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Linked {
  key: String,
  title: String,
  status: WorkflowStatus,
}

/// The linked blocking tasks of the task in the form, split into those that block Run and those
/// that no longer do.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunBlockers {
  unfinished: Vec<Linked>,
  finished: Vec<Linked>,
}
impl lurq::app::component::DevtoolsInspectable for RunBlockers {}

impl RunBlockers {
  /// The links of `organization`, `unfinished` being those whose task has not finished.
  #[must_use]
  pub fn of(tasks: &ProjectTasks, organization: &TaskOrganization, unfinished: &[TaskId]) -> Self {
    let mut blockers = Self::default();
    for link in organization.dependencies() {
      let Some(task) = tasks.tasks.iter().find(|task| task.id() == link.task) else {
        continue;
      };
      let linked = Linked {
        key: task.key().to_string(),
        title: task.details().title.as_str().to_owned(),
        status: task.organization().status(),
      };
      if unfinished.contains(&link.task) {
        blockers.unfinished.push(linked);
      } else {
        blockers.finished.push(linked);
      }
    }
    blockers
  }
}

/// The Run unavailable notice and its notes, or nothing while no link blocks Run; `flagged` adds
/// how the manual Blocked flag is cleared.
pub fn run_unavailable(ctx: &Ctx, blockers: &RunBlockers, flagged: bool) -> Option<Element> {
  let notice = match blockers.unfinished.as_slice() {
    [] => return None,
    [task] => ctx.t_args(
      "tasks.blocked.runUnavailableOne",
      [
        ("task", task.key.as_str()),
        ("title", task.title.as_str()),
        ("status", ctx.t(workflow_copy(task.status).1).as_ref()),
      ],
    ),
    tasks => ctx.t_args(
      "tasks.blocked.runUnavailableMany",
      [(
        "tasks",
        tasks
          .iter()
          .map(|task| {
            ctx
              .t_args(
                "tasks.blocked.blockerLabel",
                [("task", task.key.as_str()), ("title", task.title.as_str())],
              )
              .to_string()
          })
          .collect::<Vec<_>>()
          .join(&ctx.t("common.listSeparator"))
          .as_str(),
      )],
    ),
  };
  let note = |text: &str| {
    Text::new(text)
      .variant(TypographyStyle::Caption)
      .width(Dimension::Pct(100.0))
  };
  let column = Column::new()
    .id("task-run-unavailable")
    .width(Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Md)
    .child(organization_notice(
      &ctx.t("tasks.blocked.runUnavailableTitle"),
      &notice,
    ))
    .with_children(blockers.finished.iter().map(|task| {
      note(&ctx.t_args(
        "tasks.blocked.finishedLink",
        [
          ("task", task.key.as_str()),
          ("status", ctx.t(workflow_copy(task.status).1).as_ref()),
        ],
      ))
    }))
    .with_children(flagged.then(|| note(&ctx.t("tasks.blocked.unblockHint"))));
  Some(column.into())
}
