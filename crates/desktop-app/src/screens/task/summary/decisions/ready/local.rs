use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::integration::local::LocalPhase;
use orchester_workspace::{background, tasks::local_integration::pending_restoration};

use super::TaskContext;
use crate::{
  navigation::Navigation,
  routes::{Route, TaskView},
  ui::{
    button::{ActionButton, ButtonKind},
    notice::{Notice, NoticeTone},
  },
};

/// The merge stands while restoration remains pending. Summary keeps an entry to the durable operation.
pub(super) fn reminder(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let run = task.detail.run.as_ref()?.id();
  let root = task.project.open.repo_root.clone();
  let revision = task.project.feed.revision.get();
  let phase = ctx
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
    .get()
    .data
    .flatten()?;
  let route = Route::Project(task.project.open.project.id(), task.page(TaskView::LocalIntegration));
  let navigation = Navigation::of(ctx);
  Some(
    Notice::new("task-local-recovery", NoticeTone::Warning, ctx.t(phase))
      .body(ctx.t("local_integration.restoreBody"))
      .action(
        ActionButton::new(
          "task-open-local-recovery",
          ButtonKind::Primary,
          ctx.t("local_integration.title"),
        )
        .opens(navigation.to(route)),
      )
      .into(),
  )
}
