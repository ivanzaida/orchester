//! *Runs using this profile now* beside a profile's page (`KKd0N` in 01c): each run's task, where it works and since
//! when, and the version it keeps until it ends; a row opens its task.
use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_workspace::{agents::usage::ActiveUse, display};

use super::editor::Props;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskView},
  screens::hub::parts::{Entry, boxed, group, head},
  ui::pressable::Press,
};

pub fn render(ctx: &mut Ctx, props: &Props) -> Element {
  let active: Vec<&ActiveUse> = props
    .listed
    .usage
    .active
    .iter()
    .filter(|r| props.entry.as_ref().is_some_and(|p| p.id == r.profile))
    .collect();
  let rows: Vec<Element> = if active.is_empty() {
    vec![Entry::new("agent-active-none", ctx.t("agents.noActive")).into()]
  } else {
    let navigation = Navigation::of(ctx);
    active
      .iter()
      .enumerate()
      .map(|(index, run)| {
        let route = Route::Project(
          props.project.open.project.id(),
          ProjectPage::Task(run.task.clone(), TaskView::Summary),
        );
        let title = ctx.t_args(
          "agents.activeTask",
          [("key", run.task.to_string()), ("title", run.title.to_string())],
        );
        Entry::new(format!("agent-active-{index}"), title)
          .hint(metadata(ctx, run))
          .press(Press::Open(navigation.to(route)))
          .elsewhere()
          .into()
      })
      .collect()
  };
  group(
    "agent-active",
    head(
      "agent-active",
      &ctx.t("agents.activeTitle"),
      Some(&active.len().to_string()),
      None,
    ),
    boxed("agent-active-rows", rows),
  )
  .into()
}

/// `Feature delivery v3 · 03 Implement · started 16:12 · Keeps v4 until it ends`.
fn metadata(ctx: &Ctx, run: &ActiveUse) -> Arc<str> {
  let args = [
    ("pipeline", run.pipeline.to_string()),
    ("version", run.pipeline_version.get().to_string()),
    ("position", format!("{:02}", run.position)),
    ("step", run.step.to_string()),
    ("profile", run.version.get().to_string()),
  ];
  match run.started_at {
    Some(at) => ctx.t_args(
      "agents.activeWhereStarted",
      args.into_iter().chain([("time", display::clock(at))]),
    ),
    None => ctx.t_args("agents.activeWhere", args),
  }
}
