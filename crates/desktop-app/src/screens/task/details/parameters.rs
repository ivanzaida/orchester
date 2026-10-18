//! The Details' side sections (`Task and run parameters` `j9Jzun` in `UxsRH`): the setup the task's latest
//! run was captured with (`Execution setup · selected run` `qWSNJ`: its pipeline version and the limits it
//! started with), apart from the task's organization as it is now (`Current task organization` `V9hfw`:
//! status, priority, tags, what it depends on and when it was created). Before the first run the setup is
//! what the run would start with now, said so; a run that cannot be read has its setup *Not recorded*, never
//! taken from the task.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  task::requirements::LatestRun,
  tokens::{TokenBudget, compact_tokens},
};
use orchester_workspace::{
  budget::whole_minutes,
  tasks::{data::ProjectTasks, detail::RunView, list::PipelineLabel},
};

use crate::{
  screens::task::{TaskContext, summary::runs::line::day_and_clock},
  theme::TaskText,
  ui::{
    detail_row::{DetailValue, detail_row},
    message::message_text,
    status::{priority_look, workflow_key},
  },
};

/// The run's setup, then the task's organization.
pub fn sections(ctx: &mut Ctx, task: &TaskContext) -> Vec<Element> {
  let tasks = task.project.feed.tasks.get();
  let tasks = tasks.ready();
  vec![setup(ctx, task, tasks), organization(ctx, task, tasks)]
}

/// A section's heading and what its values are, over its rows.
fn section(id: &str, (heading, note): (&str, &str), rows: Vec<Element>) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(
      Text::new(heading)
        .id(format!("{id}-title"))
        .variant(TaskText::DetailsHeading.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(
      Text::new(note)
        .id(format!("{id}-note"))
        .variant(TaskText::DetailsNote.role())
        .width(Dimension::Pct(100.0)),
    )
    .with_children(rows)
    .into()
}

fn row(ctx: &Ctx, (id, key): (&str, &str), value: &str) -> Element {
  detail_row(id, &ctx.t(key), value, DetailValue::Text).into()
}

fn setup(ctx: &Ctx, task: &TaskContext, tasks: Option<&ProjectTasks>) -> Element {
  let detail = &task.detail;
  let id = "task-details-setup";
  let (heading, note, values) = match (detail.requirements.latest_run(), detail.run.as_ref()) {
    (LatestRun::Run(started), Some(run)) if started.run == run.id() => (
      ctx.t_args("details.setup.run", [("run", started.number.get().to_string())]),
      ctx.t("details.setup.captured"),
      run_values(ctx, run),
    ),
    (LatestRun::None, _) => (
      ctx.t("details.setup.first"),
      ctx.t("details.setup.notStarted"),
      first_values(ctx, task, tasks),
    ),
    _ => {
      let unknown = ctx.t("details.setup.notRecorded");
      (
        ctx.t("details.setup.unknown"),
        ctx.t("details.setup.unreadable"),
        [unknown.clone(), unknown.clone(), unknown],
      )
    }
  };
  let [pipeline, tokens, time] = values;
  let rows = vec![
    row(
      ctx,
      ("task-details-setup-pipeline", "details.setup.pipeline"),
      &pipeline,
    ),
    row(ctx, ("task-details-setup-tokens", "details.setup.tokens"), &tokens),
    row(ctx, ("task-details-setup-time", "details.setup.time"), &time),
  ];
  section(id, (&heading, &note), rows)
}

/// The pipeline version `run` pinned and the limits it was created with.
fn run_values(ctx: &Ctx, run: &RunView) -> [Arc<str>; 3] {
  let version = run.version.to_string();
  let pipeline = if run.pipeline.is_empty() {
    ctx.t_args("details.setup.pipelineGone", [("version", version)])
  } else {
    ctx.t_args(
      "details.setup.pipelineValue",
      [("pipeline", run.pipeline.to_string()), ("version", version)],
    )
  };
  let created = run.run.created();
  [pipeline, tokens(ctx, created.budget), time(ctx, created.max_duration)]
}

/// What the task's first run would start with now: its pipeline's published version and its limits.
fn first_values(ctx: &Ctx, task: &TaskContext, tasks: Option<&ProjectTasks>) -> [Arc<str>; 3] {
  let detail = &task.detail;
  let details = detail.task.details();
  let published = details
    .pipeline
    .and_then(|chosen| tasks?.pipelines.iter().find(|pipeline| pipeline.id == chosen).cloned());
  let pipeline = match (&detail.missing_pipeline, published) {
    (Some(slug), _) => message_text(
      ctx,
      &PipelineLabel::Missing {
        slug: Arc::from(slug.as_str()),
      }
      .message(),
    ),
    (None, Some(pipeline)) => ctx.t_args(
      "details.setup.pipelineValue",
      [
        ("pipeline", pipeline.name.to_string()),
        ("version", pipeline.version.to_string()),
      ],
    ),
    (None, None) => ctx.t("details.setup.notChosen"),
  };
  [
    pipeline,
    tokens(ctx, details.limits.budget()),
    time(ctx, details.limits.max_duration()),
  ]
}

fn tokens(ctx: &Ctx, budget: Option<TokenBudget>) -> Arc<str> {
  budget.map_or_else(
    || ctx.t("details.setup.notSet"),
    |budget| ctx.t_args("details.setup.tokensValue", [("budget", compact_tokens(budget.get()))]),
  )
}

fn time(ctx: &Ctx, limit: Option<Duration>) -> Arc<str> {
  limit.map_or_else(
    || ctx.t("details.setup.notSet"),
    |limit| ctx.t_args("details.setup.minutes", [("minutes", whole_minutes(limit).to_string())]),
  )
}

/// The task's organization as it is now; tags and dependencies by name once the project's tasks are read.
fn organization(ctx: &Ctx, task: &TaskContext, tasks: Option<&ProjectTasks>) -> Element {
  let current = &task.detail.task;
  let organization = current.organization();
  let separator = ctx.t("details.organization.separator");
  let listed = |names: Option<Vec<String>>| match names {
    None => ctx.t("details.organization.notKnown"),
    Some(names) if names.is_empty() => ctx.t("details.organization.none"),
    Some(names) => Arc::from(names.join(&separator)),
  };
  let tags = tasks.map(|tasks| {
    tasks
      .tags
      .iter()
      .filter(|tag| organization.tags().contains(&tag.id()))
      .map(|tag| tag.name().as_str().to_owned())
      .collect()
  });
  let dependencies = tasks.and_then(|tasks| {
    organization
      .dependencies()
      .iter()
      .map(|link| {
        let blocking = tasks.tasks.iter().find(|task| task.id() == link.task)?;
        Some(blocking.key().to_string())
      })
      .collect::<Option<Vec<String>>>()
  });
  let rows = vec![
    row(
      ctx,
      ("task-details-organization-status", "details.organization.status"),
      &ctx.t(workflow_key(organization.status())),
    ),
    row(
      ctx,
      ("task-details-organization-priority", "details.organization.priority"),
      &ctx.t(priority_look(organization.priority()).1),
    ),
    row(
      ctx,
      ("task-details-organization-tags", "details.organization.tags"),
      &listed(tags),
    ),
    row(
      ctx,
      ("task-details-organization-depends", "details.organization.dependsOn"),
      &listed(dependencies),
    ),
    row(
      ctx,
      ("task-details-organization-created", "details.organization.created"),
      &day_and_clock(ctx, current.created_at()),
    ),
  ];
  let heading = ctx.t("details.organization.title");
  let note = ctx.t("details.organization.current");
  section("task-details-organization", (&heading, &note), rows)
}
