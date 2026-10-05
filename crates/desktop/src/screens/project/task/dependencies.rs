use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  node::Element,
};
use orchester_core::{id::TaskId, task::Task};

use super::super::ProjectNav;
use crate::{
  background,
  messages::Message,
  navigation::route::Page,
  runner::Runners,
  tasks::{
    data::{ProjectTasks, SaveTarget},
    organization::workflow_label,
    run::{queue_organized_in, start_messages},
  },
  ui::{button::ActionButton, field::field_error, icon::Icon, message_text},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DependencyProps {
  pub root: PathBuf,
  pub task: Task,
  pub revision: (u64, u64),
}
impl lurq::app::component::DevtoolsInspectable for DependencyProps {}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Started(TaskId);
impl lurq::app::component::DevtoolsInspectable for Started {}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Problems(Arc<[Message]>);
impl lurq::app::component::DevtoolsInspectable for Problems {}

pub(super) struct TaskDependencies;
impl Component for TaskDependencies {
  type Props = DependencyProps;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<DependencyProps>().clone();
    let action = action(ctx, &props);
    // Registered once: only a start that has just succeeded opens the task again; a failed start
    // keeps the data of an earlier one that succeeded.
    let nav = ctx.use_context::<ProjectNav>().expect("shell provides navigation");
    ctx.watch(&action.state(), move |state| {
      if let Some(Started(task)) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
        nav.tasks_changed();
        nav.show(Page::task(*task));
      }
    });
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DependencyProps>().clone();
    let action = action(ctx, &props);
    let load = ctx.future((props.root.clone(), props.revision), |(root, _)| async move {
      background::run(move || ProjectTasks::load(&root))
        .await?
        .map_err(|error| crate::messages::store_error(&error))
    });
    let nav = ctx.use_context::<ProjectNav>().expect("shell provides navigation");
    let state = load.state().get();
    let mut column = Column::new()
      .id("task-dependency-readiness")
      .spacing(SpacingSize::Sm)
      .child(Text::new(&ctx.t("tasks.blocked.readinessHeading")).variant(TypographyStyle::Label));
    if let Some(error) = state.error {
      return column.child(field_error(&message_text(ctx, &error)));
    }
    let Some(tasks) = state.data else {
      return column.child(Text::new(&ctx.t("tasks.tags.loading")).variant(TypographyStyle::Caption));
    };
    column = column.child(dependency_links(ctx, &props.task, &tasks, &nav));
    let blocking = tasks
      .blocking
      .get(&props.task.id())
      .is_some_and(|links| !links.is_empty());
    let complete = props.task.details().missing().is_empty()
      && tasks
        .pipelines
        .iter()
        .any(|pipeline| Some(pipeline.id) == props.task.details().pipeline);
    let pending = action.state().get().is_pending();
    let start = action.clone();
    column = column.child(
      ActionButton::new(ctx.t(if pending {
        "tasks.new.starting"
      } else {
        "tasks.selection.run"
      }))
      .icon(Icon::Play)
      .id("task-run")
      .on_click(move || start.run(()))
      .enabled(!blocking && complete && !pending)
      .render(ctx),
    );
    if let Some(Problems(errors)) = action.state().get().error {
      column = column.with_children(errors.iter().map(|error| field_error(&message_text(ctx, error))));
    }
    column
  }
}

/// Queues the task and starts its run; the call in `create` and every `render` refer to the same
/// action, pointed at the task as last shown.
fn action(ctx: &mut Ctx, props: &DependencyProps) -> lurq::app::ctx::FutureAction<(), Started, Problems> {
  let runners = ctx.use_context::<Runners>().expect("root provides runners");
  let servers = crate::sharing::feed::ServerFeed::of(ctx).servers;
  let root = props.root.clone();
  let task = props.task.clone();
  ctx.future_action(move |()| {
    let root = root.clone();
    let runners = runners.clone();
    let task = task.clone();
    let servers = servers.clone();
    async move {
      let claims = (servers.clone(), root.clone());
      let queued = background::run(move || {
        let tasks =
          ProjectTasks::load(&root).map_err(|error| Problems(vec![crate::messages::store_error(&error)].into()))?;
        let version = tasks
          .pipelines
          .iter()
          .find(|pipeline| Some(pipeline.id) == task.details().pipeline)
          .ok_or_else(|| Problems(vec![Message::new("runs.preflight.pipelineNotPublished")].into()))?
          .version;
        queue_organized_in(
          &root,
          &servers,
          SaveTarget::Task(task.id(), task.revision()),
          task.details().clone(),
          // Nothing changed here: what is stored stays.
          (task.organization(), task.organization()),
          version,
        )
        .map_err(|error| Problems(error.messages().into()))
      })
      .await
      .map_err(|error| Problems(vec![error].into()))??;
      let task = queued.task.id();
      // Work on a shared task starts only under the person's claim.
      claims
        .0
        .claim_for_work(&claims.1, task)
        .await
        .map_err(|refusal| Problems(vec![refusal.message()].into()))?;
      let started = runners.start_run(task.first_run_key(), task).await;
      if started.is_err() {
        claims.0.release_unused_claim(&claims.1, task).await;
      }
      started.map_err(|error| Problems(start_messages(&error).into()))?;
      Ok(Started(task))
    }
  })
}

fn dependency_links(ctx: &Ctx, task: &Task, tasks: &ProjectTasks, nav: &ProjectNav) -> Element {
  let mut column = Column::new().spacing(SpacingSize::Sm);
  let unfinished = tasks.blocking.get(&task.id()).map(Vec::as_slice).unwrap_or_default();
  for link in task.organization().dependencies() {
    if let Some(blocker) = tasks.tasks.iter().find(|task| task.id() == link.task) {
      column = column
        .child(
          ActionButton::new(ctx.t_args(
            "tasks.blocked.linkLabel",
            [
              ("key", blocker.key().to_string().as_str()),
              ("title", blocker.details().title.as_str()),
            ],
          ))
          .icon(Icon::Link)
          .link(nav.link(Page::task(blocker.id())))
          .render(ctx),
        )
        .child(workflow_label(ctx, blocker.organization().status()));
    }
  }
  if !unfinished.is_empty() {
    let keys = unfinished
      .iter()
      .filter_map(|id| tasks.tasks.iter().find(|task| task.id() == *id))
      .map(|task| task.key().to_string())
      .collect::<Vec<_>>()
      .join(&ctx.t("common.listSeparator"));
    // The reason is the page's Blocked notice; here only why Run is unavailable (`Ncjvt`).
    column = column.child(
      Text::new(&ctx.t_args("tasks.blocked.runUnavailableWhile", [("tasks", keys.as_str())]))
        .variant(TypographyStyle::Description),
    );
  } else {
    column = column.child(Text::new(&ctx.t("tasks.readiness.ready")).variant(TypographyStyle::Caption));
  }
  column.into()
}
