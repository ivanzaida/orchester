//! The Overview's notices. Over its heading, while the project's runner could not restart, a failure
//! notice with *Open runner log* and *Restart runner*, as the status bar offers them (`Notice · The runner
//! could not restart` `Vfh7n`). Under its heading, a notice for each task other tasks block, saying what
//! it waits for (`Notice · Blocked · KON-7` `D9aJnc`), and for each task whose latest run cannot be read
//! (`Notice · No run to show for KON-21` `pS4Z3`), each opening its task.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  messages,
  runner::state::RunnerState,
  tasks::{
    board::{self, NoticeKind},
    data::ProjectTasks,
  },
};

use super::open_task;
use crate::{
  project::ShownProject,
  routes::TaskTab,
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

/// The runner's notice, while it gave up starting: why, and what can be done.
pub(super) fn runner(ctx: &mut Ctx, project: &ShownProject) -> Option<Element> {
  let root = project.open.repo_root.clone();
  let state = project.feed.status.get()?.state_for(&root);
  let RunnerState::Failed { cause, .. } = &state else {
    return None;
  };
  let reason = message_text(ctx, &messages::runner_failure(cause));
  let body = ctx.t_args("overview.runner.body", [("reason", reason)]);
  let runners = Services::of(ctx).runners;
  let log = runners.clone();
  let open_log = ActionButton::new(
    "overview-runner-log",
    ButtonKind::Secondary,
    ctx.t("overview.runner.openLog"),
  )
  .icon(Icon::FileText)
  .on_click(move || log.open_log_of(&root));
  let restart = ActionButton::new(
    "overview-runner-restart",
    ButtonKind::Primary,
    ctx.t("overview.runner.restart"),
  )
  .icon(Icon::RotateCcw)
  .on_click(move || runners.restart());
  Some(
    Notice::new(
      "overview-runner-notice",
      NoticeTone::Danger,
      ctx.t("overview.runner.title"),
    )
    .body(body)
    .action(open_log)
    .action(restart)
    .into(),
  )
}

/// The notices of tasks other tasks block and of tasks whose latest run cannot be read, 8 apart; `None`
/// when there are none.
pub(super) fn tasks(ctx: &mut Ctx, project: ProjectId, tasks: &ProjectTasks) -> Option<Element> {
  let separator = ctx.t("overview.listSeparator");
  let notices = board::notices(tasks, &separator);
  if notices.is_empty() {
    return None;
  }
  let shown: Vec<Element> = notices
    .iter()
    .map(|notice| {
      // A task can be both blocked and unreadable: the kind keeps their ids apart.
      let (glyph, label, kind) = match notice.kind {
        NoticeKind::Blocked => (
          Icon::OctagonPause,
          ctx.t_args("overview.notices.open", [("task", &*notice.key)]),
          "blocked",
        ),
        // `circle-help`, as the design draws it (`pS4Z3` in `Bq82T`).
        NoticeKind::Unreadable => (Icon::CircleHelp, ctx.t("overview.notices.openTask"), "unreadable"),
      };
      let id = format!("overview-notice-{}-{kind}", notice.key);
      let open = ActionButton::new(format!("{id}-open"), ButtonKind::Secondary, label).on_click(open_task(
        ctx,
        project,
        &notice.key,
        TaskTab::Summary,
      ));
      Notice::new(id, NoticeTone::Info, message_text(ctx, &notice.title))
        .icon(glyph)
        .body(message_text(ctx, &notice.body))
        .action(open)
        .into()
    })
    .collect();
  Some(
    Column::new()
      .id("overview-notices")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .with_children(shown)
      .into(),
  )
}
