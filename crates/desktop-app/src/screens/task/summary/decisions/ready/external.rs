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

pub(super) fn notice(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let run = task.detail.run.as_ref()?.id();
  let root = task.project.open.repo_root.clone();
  let revision = task.project.feed.revision.get();
  let evidence = ctx
    .future((root, run, revision), |(root, run, _)| async move {
      background::run(move || external_integration(&root, run).map(Evidence).map_err(|_| ()))
        .await
        .map_err(|_| ())?
    })
    .state()
    .get()
    .data?
    .0?;
  let route = Route::Project(
    task.project.open.project.id(),
    ProjectPage::Task(task.key.clone(), TaskView::LocalIntegration),
  );
  let navigation = Navigation::of(ctx);
  Some(
    Notice::new(
      "task-external-integration",
      NoticeTone::Warning,
      ctx.t("local_integration.externalTitle"),
    )
    .body(ctx.t_args(
      "local_integration.externalEvidence",
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
    )
    .into(),
  )
}
