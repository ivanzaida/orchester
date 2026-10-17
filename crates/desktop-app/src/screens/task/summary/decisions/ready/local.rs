use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::integration::local::LocalPhase;
use orchester_workspace::{background, tasks::local_integration::pending_restoration};

use super::TaskContext;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskView},
  ui::{
    button::{ActionButton, ButtonKind},
    notice::{Notice, NoticeTone},
  },
};

/// The merge stands while restoration remains pending. Summary keeps an entry to the durable operation. With it,
/// whether reading it has settled: reads that run git in the checkout wait for it ([`super::pull_request::read`]).
pub(super) fn reminder(ctx: &mut Ctx, task: &TaskContext) -> (Option<Element>, bool) {
  let Some(run) = task.detail.run.as_ref().map(|run| run.id()) else {
    return (None, true);
  };
  let root = task.project.open.repo_root.clone();
  let revision = task.project.feed.revision.get();
  let state = ctx
    .future((root, run, revision), |(root, run, _)| async move {
      background::run(move || {
        pending_restoration(&root, run).map(|phase| {
          phase.map(|phase| match phase {
            LocalPhase::RestorationVerified => "local_integration.restored",
            LocalPhase::MergedAwaitingRestore | LocalPhase::Restoring | LocalPhase::RestoreConflicts => {
              "local_integration.restoreTitle"
            }
            _ => "local_integration.recoveryTitle",
          })
        })
      })
      .await
      .map_err(|_| ())?
      .map_err(|_| ())
    })
    .state()
    .get();
  let settled = !state.is_pending() && (state.data.is_some() || state.error.is_some());
  let Some(phase) = state.data.flatten() else {
    return (None, settled);
  };
  let route = Route::Project(
    task.project.open.project.id(),
    ProjectPage::Task(task.key.clone(), TaskView::LocalIntegration),
  );
  let navigation = Navigation::of(ctx);
  let notice = Notice::new("task-local-recovery", NoticeTone::Warning, ctx.t(phase))
    .body(ctx.t("local_integration.restoreBody"))
    .action(
      ActionButton::new(
        "task-open-local-recovery",
        ButtonKind::Primary,
        ctx.t("local_integration.title"),
      )
      .on_click(move || navigation.go(route.clone())),
    );
  (Some(notice.into()), settled)
}
