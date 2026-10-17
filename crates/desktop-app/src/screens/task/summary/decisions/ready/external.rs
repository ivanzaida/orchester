use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  node::Element,
};
use orchester_core::integration::local::ContainedResult;
use orchester_workspace::{background, tasks::local_integration::external_integration};

use super::TaskContext;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskView},
  ui::{
    button::{ActionButton, ButtonKind},
    notice::{Notice, NoticeTone},
  },
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Evidence(Option<ContainedResult>);
impl DevtoolsInspectable for Evidence {}

/// The notice of current external Git evidence, if any, and whether reading it has settled: reads that run git in
/// the checkout wait for it ([`super::pull_request::read`]).
pub(super) fn notice(ctx: &mut Ctx, task: &TaskContext) -> (Option<Element>, bool) {
  let Some(run) = task.detail.run.as_ref().map(|run| run.id()) else {
    return (None, true);
  };
  let root = task.project.open.repo_root.clone();
  let revision = task.project.feed.revision.get();
  let state = ctx
    .future((root, run, revision), |(root, run, _)| async move {
      background::run(move || external_integration(&root, run).map(Evidence).map_err(|_| ()))
        .await
        .map_err(|_| ())?
    })
    .state()
    .get();
  let settled = !state.is_pending() && (state.data.is_some() || state.error.is_some());
  let Some(evidence) = state.data.and_then(|evidence| evidence.0) else {
    return (None, settled);
  };
  let route = Route::Project(
    task.project.open.project.id(),
    ProjectPage::Task(task.key.clone(), TaskView::LocalIntegration),
  );
  let navigation = Navigation::of(ctx);
  let notice = Notice::new(
    "task-external-integration",
    NoticeTone::Warning,
    ctx.t("local_integration.externalTitle"),
  )
  .body(ctx.t_args(
    crate::screens::task::local_integration::words::evidence_body(evidence.kind),
    [
      ("source", evidence.source.as_str()),
      ("target", evidence.target.as_str()),
    ],
  ))
  .action(
    ActionButton::new(
      "task-inspect-external-integration",
      ButtonKind::Secondary,
      ctx.t("local_integration.title"),
    )
    .on_click(move || navigation.go(route.clone())),
  );
  (Some(notice.into()), settled)
}
