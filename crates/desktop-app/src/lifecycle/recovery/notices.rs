//! The recovery's other notices: the runs that did not continue, each with why, and what could not be
//! read. Both can be put away with *Hide*; the runs stay as they are and their task pages offer them.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::id::ProjectId;
use orchester_workspace::recovery::{Continuation, Recovery, Unread};

use super::NOTICE_ID;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskTab, TaskView},
  ui::{
    button::{ActionButton, ButtonKind},
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

/// *1 of 2 runs continued*, naming each run that did not continue with why; they stay interrupted. The
/// first one's task opens from it.
pub(super) fn not_continued(
  ctx: &mut Ctx,
  (recovery, project): (&Recovery, ProjectId),
  continuation: &Continuation,
) -> Element {
  let left = &continuation.not_continued;
  let count = continuation.continued + left.len();
  let title = ctx.t_args(
    "lifecycle.recovery.partial.title",
    [
      ("continued", continuation.continued.to_string()),
      ("count", count.to_string()),
    ],
  );
  let mut lines: Vec<String> = left
    .iter()
    .map(|left| {
      ctx
        .t_args(
          "lifecycle.recovery.partial.run",
          [
            ("task", left.run.task_key.to_string()),
            ("reason", message_text(ctx, &left.reason).to_string()),
          ],
        )
        .to_string()
    })
    .collect();
  lines.push(ctx.t("lifecycle.recovery.partial.stays").to_string());
  let mut notice = Notice::new(NOTICE_ID, NoticeTone::Warning, title).body(lines.join("\n"));
  if let Some(first) = left.first() {
    let navigation = Navigation::of(ctx);
    let route = Route::Project(
      project,
      ProjectPage::Task(first.run.task_key.clone(), TaskView::of(TaskTab::Summary)),
    );
    let task = first.run.task_key.to_string();
    notice = notice.action(
      ActionButton::new(
        "recovery-open-task",
        ButtonKind::Secondary,
        ctx.t_args("lifecycle.recovery.partial.open", [("task", task.as_str())]),
      )
      .on_click(move || navigation.go(route.clone())),
    );
  }
  notice.action(hide(ctx, recovery, project)).into()
}

/// What could not be read: its lines under a title, said rather than left out.
pub(super) fn unread(ctx: &Ctx, (recovery, project): (&Recovery, ProjectId), lines: &[impl AsRef<str>]) -> Element {
  let lines: Vec<&str> = lines.iter().map(AsRef::as_ref).collect();
  Notice::new(NOTICE_ID, NoticeTone::Warning, ctx.t("lifecycle.recovery.unread.title"))
    .body(ctx.t_args("lifecycle.recovery.unread.body", [("reasons", lines.join("\n"))]))
    .action(hide(ctx, recovery, project))
    .into()
}

/// A line for each run, or the project's runs, that could not be read, with why.
pub(super) fn unread_lines(ctx: &Ctx, unread: &[Unread]) -> Vec<Arc<str>> {
  unread
    .iter()
    .map(|unread| {
      let reason = message_text(ctx, &unread.reason).to_string();
      match (&unread.task_key, unread.run) {
        (Some(task), _) => ctx.t_args(
          "lifecycle.recovery.unread.runOf",
          [("task", task.to_string()), ("reason", reason)],
        ),
        (None, Some(_)) => ctx.t_args("lifecycle.recovery.unread.run", [("reason", reason)]),
        (None, None) => ctx.t_args("lifecycle.recovery.unread.project", [("reason", reason)]),
      }
    })
    .collect()
}

/// *Hide*: the notice goes; the runs stay as they are.
fn hide(ctx: &Ctx, recovery: &Recovery, project: ProjectId) -> ActionButton {
  let recovery = recovery.clone();
  ActionButton::new(
    "recovery-put-away",
    ButtonKind::Secondary,
    ctx.t("lifecycle.recovery.hide"),
  )
  .on_click(move || recovery.leave(project))
}
