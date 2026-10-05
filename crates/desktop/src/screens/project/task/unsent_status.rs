//! A run's workflow status Orchester did not send to the server of a shared task, quietly on the
//! task until the person dismisses it: the server's status changed meanwhile and stays, or the
//! person no longer holds the task.

use std::path::PathBuf;

use lurq::{
  app::{component::Component, ctx::Ctx},
  node::Element,
};
use orchester_core::id::TaskId;
use orchester_store::{NotSent, ProjectDb, UnsentStatus};

use super::super::ProjectNav;
use crate::{
  background, display,
  messages::{self, Message},
  tasks::organization::workflow_copy,
  ui::{button::ActionButton, icon::Icon, notice::Notice},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct UnsentProps {
  pub root: PathBuf,
  pub task: TaskId,
  pub unsent: UnsentStatus,
}

impl lurq::app::component::DevtoolsInspectable for UnsentProps {}

pub(super) struct UnsentStatusNotice;

impl Component for UnsentStatusNotice {
  type Props = UnsentProps;

  fn create(ctx: &mut Ctx) -> Self {
    let dismiss = dismiss_action(ctx);
    let nav = ctx.use_context::<ProjectNav>().expect("shell provides navigation");
    ctx.watch(&dismiss.state(), move |state| {
      if state.is_fulfilled() {
        nav.tasks_changed();
      }
    });
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<UnsentProps>().clone();
    let dismiss = dismiss_action(ctx);
    let status = ctx.t(workflow_copy(props.unsent.status).1);
    let time = display::clock(props.unsent.at);
    let detail = match props.unsent.reason {
      NotSent::Changed { current } => ctx.t_args(
        "tasks.unsentStatus.changed",
        [
          ("status", &*status),
          ("time", time.as_str()),
          ("current", &*ctx.t(workflow_copy(current).1)),
        ],
      ),
      NotSent::NotHolder => ctx.t_args(
        "tasks.unsentStatus.notHolder",
        [("status", &*status), ("time", time.as_str())],
      ),
      NotSent::Refused => ctx.t_args(
        "tasks.unsentStatus.refused",
        [("status", &*status), ("time", time.as_str())],
      ),
    };
    let (root, task) = (props.root.clone(), props.task);
    Notice::new(Icon::CloudOff, ctx.t("tasks.unsentStatus.heading"), detail)
      .id("task-unsent-status")
      .action(
        ActionButton::new(ctx.t("tasks.unsentStatus.dismiss"))
          .id("task-unsent-status-dismiss")
          .on_click(move || dismiss.run((root.clone(), task)))
          .render(ctx),
      )
  }
}

type DismissAction = lurq::app::ctx::FutureAction<(PathBuf, TaskId), (), Message>;

fn dismiss_action(ctx: &mut Ctx) -> DismissAction {
  ctx.future_action(|(root, task): (PathBuf, TaskId)| async move {
    background::run(move || ProjectDb::open(&root)?.dismiss_unsent_status(task))
      .await?
      .map_err(|error| messages::store_error(&error))
  })
}
