//! The New task form reopened on a saved task: Edit task (`E1o77O`, after a change `ctHXE`). The
//! fields start with the saved values, *Save changes* saves over the same task and is unavailable
//! while nothing differs from it, and leaving with changes that are not saved asks first (`kDmo9`).

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  task::{Task, TaskDetails, organization::TaskOrganization},
  tokens::grouped,
};

use super::{NewTaskForm, PipelineSelection, brief::Criterion};
use crate::{
  budget::minutes_text,
  messages::{Message, edit_error},
  tasks::data::PipelineChoice,
};

/// The saved task the form edits, as the project's tasks were last read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Editing {
  pub task: Arc<Task>,
  /// A run was started for the task.
  pub has_run: bool,
}

/// A queued task without a run, opened in the locked form to retry its first run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovering {
  pub task: Arc<Task>,
  pub pipeline: Arc<PipelineChoice>,
}

/// Fills the form with the task's saved details when it can still be edited; otherwise leads to
/// the task's page, which says why. Checked once, when the form opens: a draft the form itself
/// queues with Create & run stays on the form until its run starts.
pub(super) fn open(ctx: &Ctx, form: &NewTaskForm, editing: &Editing) {
  match editing.task.check_editable(editing.has_run) {
    Ok(()) => seed(ctx, form, &editing.task),
    Err(error) => form.nav.refuse_edit(editing.task.id(), edit_error(&error)),
  }
}

/// Fills the form with `task`'s saved details; an empty criteria list keeps one empty row to type
/// in, as a new form has.
pub(super) fn seed(ctx: &Ctx, form: &NewTaskForm, task: &Task) {
  fill(ctx, form, task.details(), task.organization());
}

/// Fills the form with `details` and `organization`, e.g. what a save stored after merging the
/// person's change with someone else's.
pub(super) fn fill(ctx: &Ctx, form: &NewTaskForm, details: &TaskDetails, organization: &TaskOrganization) {
  form.organization.seed(organization);
  form.title.set(details.title.as_str().to_owned());
  form.brief.set(details.brief.clone());
  let mut criteria: Vec<Criterion> = (0..)
    .zip(&details.acceptance_criteria)
    .map(|(id, text)| Criterion::with_text(id, text))
    .collect();
  if criteria.is_empty() {
    criteria.push(Criterion::new(0));
  }
  form.criteria.set(criteria);
  form.pipeline.set(PipelineSelection(details.pipeline));
  if let Some(limits) = details.limits {
    let separator = ctx.t("number.groupSeparator");
    form.budget.set(grouped(limits.budget().get(), &separator));
    form.time_limit.set(minutes_text(limits.max_duration()));
  }
}

/// What leaving the form with changes that are not saved says (`kDmo9`).
pub(super) fn leave_question(task: &Task) -> Message {
  Message::new("tasks.edit.leave").with("task", task.key().to_string())
}

/// The topbar's breadcrumb (`K51w5h`).
pub(super) fn breadcrumb(ctx: &Ctx, project: &str, editing: Option<&Task>, recovering: Option<&Task>) -> Arc<str> {
  if let Some(task) = recovering {
    return ctx.t_args(
      "tasks.new.recoveryBreadcrumb",
      [("project", project), ("task", task.key().to_string().as_str())],
    );
  }
  match editing {
    Some(task) => ctx.t_args(
      "tasks.edit.breadcrumb",
      [("project", project), ("task", task.key().to_string().as_str())],
    ),
    None => ctx.t_args("tasks.new.breadcrumb", [("project", project)]),
  }
}

/// The heading's title and description (`Bzn0d`, `iTAZH`).
pub(super) fn heading_texts(ctx: &Ctx, editing: Option<&Task>) -> (Arc<str>, Arc<str>) {
  match editing {
    Some(task) => (
      ctx.t("tasks.edit.title"),
      ctx.t_args(
        if task.status() == orchester_core::task::TaskStatus::Draft {
          "tasks.edit.description"
        } else {
          "tasks.edit.descriptionStarted"
        },
        [("task", task.key().to_string().as_str())],
      ),
    ),
    None => (ctx.t("tasks.new.title"), ctx.t("tasks.new.description")),
  }
}
