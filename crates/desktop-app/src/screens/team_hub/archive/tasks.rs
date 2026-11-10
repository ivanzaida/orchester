//! A local pipeline's confirmation names the tasks that use it, not the resources (nothing names a pipeline but a
//! task): their count, the first few by key and title and how many more there are, and what happens to them. The store
//! refuses a run on an archived pipeline, so those tasks cannot start one until they use another pipeline or it is
//! restored.

use lurq::{
  app::ctx::Ctx,
  components::{Column, Text},
  node::dimension::Dimension,
};
use orchester_workspace::pipelines::task_use::TaskUse;

use super::dialog::{group, row};
use crate::theme::ArchiveText;

/// How many tasks the list names before it counts the rest.
const NAMED: usize = 5;

/// The tasks that use the pipeline, as the dialog's *Used by* list (`Used by` `pC90U`).
pub(super) fn used_by(ctx: &Ctx, tasks: &[TaskUse]) -> Column {
  let mut rows: Vec<_> = tasks
    .iter()
    .take(NAMED)
    .enumerate()
    .map(|(index, task)| {
      let id = format!("team-hub-archive-task-{index}");
      row(
        id.clone(),
        Text::new(&ctx.t_args(
          "local_archive.taskRow",
          [("key", task.key.as_str()), ("title", task.title.as_str())],
        ))
        .id(format!("{id}-name"))
        .variant(ArchiveText::Name.role()),
        None,
      )
      .into()
    })
    .collect();
  if let Some(rest) = tasks.len().checked_sub(NAMED).filter(|rest| *rest > 0) {
    rows.push(
      row(
        "team-hub-archive-tasks-more".to_owned(),
        Text::new(&ctx.t_args("local_archive.tasksMore", [("count", rest.to_string())]))
          .id("team-hub-archive-tasks-more-name")
          .variant(ArchiveText::Holder.role()),
        None,
      )
      .into(),
    );
  }
  let heading = ctx.t_args("team_archive.usedBy", [("count", tasks.len().to_string())]);
  group(ctx, "team-hub-archive-used", &heading, rows, "local_archive.noTasks")
}

/// What archiving does to those tasks; none when no task uses the pipeline.
pub(super) fn effect(ctx: &Ctx, name: &str, tasks: &[TaskUse]) -> Option<Text> {
  let key = match tasks.len() {
    0 => return None,
    1 => "local_archive.tasksEffectOne",
    _ => "local_archive.tasksEffect",
  };
  Some(
    Text::new(&ctx.t_args(key, [("name", name)]))
      .id("team-hub-archive-tasks-effect")
      .variant(ArchiveText::Note.role())
      .width(Dimension::Pct(100.0)),
  )
}
