use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{background, tasks::detail::RunView};

use super::{TaskContext, work::problem};
use crate::ui::{
  notice::{Notice, NoticeTone},
  skeleton::{self, Skeleton},
};

/// The checkout's facts while they are read: lines of bones, as Local integration has no loading frame.
fn reading(ctx: &mut Ctx) -> Element {
  let bones = skeleton::lines(ctx, &[360.0, 520.0, 440.0, 300.0]);
  Skeleton::new("local-target-loading", ctx.t("local_integration.loading")).show(ctx, bones)
}

#[derive(Clone, Debug, PartialEq)]
struct Recovery(Option<RunView>, orchester_core::id::TaskId);
impl lurq::app::component::DevtoolsInspectable for Recovery {}

pub(super) struct Target;
impl Component for Target {
  type Props = TaskContext;

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let mut task = ctx.props::<TaskContext>().clone();
    let revision = task.project.feed.revision.get();
    let read = ctx.future(
      (task.project.open.repo_root.clone(), task.detail.task.id(), revision),
      |(root, task, _)| async move {
        background::run(move || {
          orchester_workspace::tasks::local_integration::recovery_run(&root, task)
            .map(|run| Recovery(run, task))
            .map_err(problem)
        })
        .await
        .map_err(|_| super::work::Problem("local_integration.error.io"))?
      },
    );
    let state = read.state().get();
    let Some(Recovery(recovery, observed)) = state.data else {
      return match state.error {
        Some(reason) => Element::from(Notice::new("local-target-error", NoticeTone::Danger, ctx.t(reason.0))),
        None => reading(ctx),
      };
    };
    if observed != task.detail.task.id() {
      return reading(ctx);
    }
    let prior = recovery.filter(|run| task.detail.run.as_ref().is_none_or(|latest| latest.id() != run.id()));
    let notice = prior.as_ref().map(|run| {
      Notice::new(
        "local-prior-operation",
        NoticeTone::Warning,
        ctx.t_args(
          "local_integration.priorOperation",
          [("run", run.run.number().get().to_string())],
        ),
      )
    });
    if let Some(prior) = prior {
      let mut detail = (*task.detail).clone();
      detail.run = Some(prior);
      task.detail = Arc::new(detail);
    }
    let content = super::page(ctx, &task);
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .flex(1.0)
      .spacing(SpacingSize::Md)
      .with_children(
        state
          .error
          .map(|reason| Notice::new("local-target-error", NoticeTone::Danger, ctx.t(reason.0))),
      )
      .with_children(notice)
      .child(content)
      .into()
  }
}
