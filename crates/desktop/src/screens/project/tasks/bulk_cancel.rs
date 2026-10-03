//! What bulk Canceled asks before it stops active runs (`hFH4D`): how many tasks and runs, which
//! run on which task, and which selected tasks have none.

use std::{collections::BTreeSet, sync::Arc};

use lurq::app::ctx::Ctx;
use orchester_core::{
  id::{RunId, TaskId},
  run::TaskPhase,
};

use crate::tasks::data::ProjectTasks;

/// The confirmation's title, explanation, notice heading and text, and the Stop button's label.
pub(super) struct BulkCancelText {
  pub title: Arc<str>,
  pub explanation: Arc<str>,
  pub heading: Arc<str>,
  pub notice: Arc<str>,
  pub confirm: Arc<str>,
  /// The runs the notice names, by task: Stop stops these and no other.
  pub runs: Vec<(TaskId, RunId)>,
}

/// One selected task with a run that has not ended: its key, title and run number, when readable.
struct Active {
  task: TaskId,
  run_id: Option<RunId>,
  key: String,
  title: String,
  run: Option<u32>,
}

pub(super) fn bulk_cancel_text(ctx: &Ctx, tasks: &ProjectTasks, selected: &BTreeSet<TaskId>) -> BulkCancelText {
  let (mut active, mut idle) = (Vec::new(), Vec::new());
  for task in tasks.tasks.iter().filter(|task| selected.contains(&task.id())) {
    let latest = tasks.phases.get(&task.id());
    if latest.is_some_and(|latest| latest.phase().is_none_or(TaskPhase::run_not_ended)) {
      active.push(Active {
        task: task.id(),
        run_id: latest.and_then(|latest| latest.run()).map(|run| run.id),
        key: task.key().to_string(),
        title: task.details().title.as_str().to_owned(),
        run: latest.and_then(|latest| latest.run()).map(|run| run.number.get()),
      });
    } else {
      idle.push(task.key().to_string());
    }
  }
  let (title, explanation, confirm) = headline(ctx, selected.len(), &active);
  let (heading, notice) = notice(ctx, &active, &idle);
  BulkCancelText {
    title,
    explanation,
    heading,
    notice,
    confirm,
    runs: active
      .iter()
      .filter_map(|active| active.run_id.map(|run| (active.task, run)))
      .collect(),
  }
}

/// The title, explanation and Stop label: one task, one run among several tasks, or several runs.
fn headline(ctx: &Ctx, selected: usize, active: &[Active]) -> (Arc<str>, Arc<str>, Arc<str>) {
  let (count, runs) = (selected.to_string(), active.len().to_string());
  let first = active.first();
  let run = first
    .and_then(|active| active.run)
    .map(|run| run.to_string())
    .unwrap_or_default();
  let task = first.map(|active| active.key.as_str()).unwrap_or_default();
  if selected == 1 {
    return (
      ctx.t("tasks.cancel.bulkTitle.oneTask"),
      ctx.t_args(
        "tasks.cancel.bulkExplanation.oneTask",
        [("run", run.as_str()), ("task", task)],
      ),
      ctx.t("tasks.cancel.bulkConfirm.oneTask"),
    );
  }
  if active.len() == 1 {
    return (
      ctx.t_args("tasks.cancel.bulkTitle.oneRun", [("tasks", count.as_str())]),
      ctx.t_args(
        "tasks.cancel.bulkExplanation.oneRun",
        [("run", run.as_str()), ("task", task), ("tasks", count.as_str())],
      ),
      ctx.t_args("tasks.cancel.bulkConfirm.oneRun", [("tasks", count.as_str())]),
    );
  }
  (
    ctx.t_args(
      "tasks.cancel.bulkTitle.other",
      [("tasks", count.as_str()), ("runs", runs.as_str())],
    ),
    ctx.t_args(
      "tasks.cancel.bulkExplanation.other",
      [("runs", runs.as_str()), ("tasks", count.as_str())],
    ),
    ctx.t_args("tasks.cancel.bulkConfirm.other", [("tasks", count.as_str())]),
  )
}

/// The notice: each run that stops, on which task, then the selected tasks without one.
fn notice(ctx: &Ctx, active: &[Active], idle: &[String]) -> (Arc<str>, Arc<str>) {
  let separator = ctx.t("common.listSeparator");
  let lines: Vec<String> = active
    .iter()
    .map(|active| match active.run {
      Some(run) => ctx.t_args(
        "tasks.cancel.activeRunLine",
        [
          ("task", active.key.as_str()),
          ("title", active.title.as_str()),
          ("run", run.to_string().as_str()),
        ],
      ),
      None => ctx.t_args(
        "tasks.cancel.activeRunUnread",
        [("task", active.key.as_str()), ("title", active.title.as_str())],
      ),
    })
    .map(|line| line.to_string())
    .collect();
  let runs = ctx.t_args("tasks.cancel.activeRuns", [("runs", lines.join(&separator))]);
  let idle_tasks = match idle {
    [first, second] => Some(
      ctx
        .t_args(
          "tasks.cancel.idlePair",
          [("first", first.as_str()), ("second", second.as_str())],
        )
        .to_string(),
    ),
    [] => None,
    tasks => Some(tasks.join(&separator)),
  };
  let notice = match idle_tasks {
    None => runs,
    Some(tasks) => ctx.t_args(
      if idle.len() == 1 {
        "tasks.cancel.noticeWithIdle.one"
      } else {
        "tasks.cancel.noticeWithIdle.other"
      },
      [("runs", runs.as_ref()), ("tasks", tasks.as_str())],
    ),
  };
  let heading = ctx.t(if active.len() == 1 {
    "tasks.cancel.activeRunHeading.one"
  } else {
    "tasks.cancel.activeRunHeading.other"
  });
  (heading, notice)
}
