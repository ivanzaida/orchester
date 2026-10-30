//! The task's properties beside the Summary's column (`Properties` `Sq8ss`, 260 wide, 24 inside its left hairline):
//! a row each, 7 over and under it, what it is in 12 `text_muted` in an 84 column and its value 8 after it in 12 —
//! Status and Priority with their glyphs, Pipeline, Tags, Depends on, Limits, Branch in Geist Mono, Base and where it
//! came from (`base_branch`, `qg3JF`) and Created. They replace *Run #N setup* and *Task organization* and keep what
//! those said: the pipeline and limits are those the latest run started with, and when the task has others now the
//! value says *changed* and its tooltip what the task has; a run that cannot be read, or ran on a teammate's computer,
//! has them *Not recorded*. Then what the shared project says of the task, the run's facts (`run_facts`), and last the
//! link to the task's memory (`Memory link` `lJQcf`).

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  task::{organization::WorkflowStatus, requirements::LatestRun},
  tokens::{TokenBudget, compact_tokens},
};
use orchester_workspace::{
  budget::whole_minutes,
  tasks::{data::ProjectTasks, list::PipelineLabel},
};

use super::{run_facts, runs::line::day_and_clock};
use crate::{
  navigation::Navigation,
  routes::{Route, TaskView},
  screens::task::TaskContext,
  theme::{ExtraSpacing, IconSize, TaskSpacing},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::opening,
    status::priority_look,
    tooltip::{Tooltip, Truncated},
  },
};

/// The key column of a property's row (`Key` `z7apaf`).
const KEY_WIDTH: f32 = 84.0;

/// How a property's value is set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
  /// 12 `text_primary`: Status, Priority, Pipeline.
  Primary,
  /// 12 `text_secondary`: Tags, Limits, Created, a run's facts.
  Secondary,
  /// Geist Mono 12 `text_secondary`: Branch, a revision, a path.
  Mono,
  /// 12 `text_muted`: none, as *—*.
  Muted,
}

/// `value`, set as `look` says, as the element `id`.
pub fn value(id: &str, value: &str, look: Look) -> Text {
  let (typography, color) = match look {
    Look::Primary => (TypographyStyle::Description, PaletteColor::TextPrimary),
    Look::Secondary => (TypographyStyle::Description, PaletteColor::TextSecondary),
    Look::Mono => (TypographyStyle::Mono, PaletteColor::TextSecondary),
    Look::Muted => (TypographyStyle::Description, PaletteColor::TextMuted),
  };
  Text::new(value)
    .id(id.to_owned())
    .variant(typography)
    .color(color)
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
}

/// A property's row: `key` beside `value`, `padding` over and under them; the key's element id is `<id>-key`.
pub fn row(id: &str, key: &str, value: impl Into<Element>, padding: SpacingSize) -> Element {
  Row::new()
    .id(format!("{id}-row"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(padding))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(
      Text::new(key)
        .id(format!("{id}-key"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Px(KEY_WIDTH))
        .flex_shrink(0.0),
    )
    .child(value)
    .into()
}

/// The column: the task's properties, what the shared project says of it (`section`), the run's facts with their
/// disclosure `more`, and the memory link.
pub fn column(ctx: &mut Ctx, task: &TaskContext, (section, more): (Option<Element>, &Signal<bool>)) -> Element {
  let tasks = task.project.feed.tasks.get();
  let tasks = tasks.ready();
  let organization = task.detail.task.organization();
  let property = |id: &str, key: &str, value: Element| row(id, key, value, TaskSpacing::PropertyRow.role());
  let rows = vec![
    property(
      "task-property-status",
      &ctx.t("summary.properties.status"),
      status(ctx, organization.status()),
    ),
    property(
      "task-property-priority",
      &ctx.t("summary.properties.priority"),
      priority(ctx, task),
    ),
    property(
      "task-property-pipeline",
      &ctx.t("summary.properties.pipeline"),
      pipeline(ctx, task, tasks),
    ),
    property(
      "task-property-tags",
      &ctx.t("summary.properties.tags"),
      tags(ctx, task, tasks),
    ),
    property(
      "task-property-depends",
      &ctx.t("summary.properties.dependsOn"),
      depends(ctx, task, tasks),
    ),
    property(
      "task-property-limits",
      &ctx.t("summary.properties.limits"),
      limits(ctx, task),
    ),
    property(
      "task-property-branch",
      &ctx.t("summary.properties.branch"),
      branch(ctx, task),
    ),
    property(
      "task-property-base",
      &ctx.t("summary.properties.base.label"),
      super::base_branch::base(ctx, task, tasks),
    ),
    property(
      "task-property-created",
      &ctx.t("summary.properties.created"),
      value(
        "task-property-created",
        &day_and_clock(ctx, task.detail.task.created_at()),
        Look::Secondary,
      )
      .into(),
    ),
  ];
  let facts = task
    .detail
    .run
    .as_ref()
    .map(|run| run_facts::group(ctx, task, run, more));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .with_children(rows)
    .with_children(section.map(|section| {
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::vertical(SpacingSize::Md))
        .child(section)
    }))
    .with_children(facts)
    .child(memory_link(ctx, task))
    .into()
}

/// A glyph and its word.
fn labelled(id: &str, (glyph, color): (Icon, PaletteColor), word: &str) -> Element {
  Row::new()
    .id(id.to_owned())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, color))
    .child(value(&format!("{id}-label"), word, Look::Primary))
    .into()
}

fn status(ctx: &Ctx, status: WorkflowStatus) -> Element {
  let glyph = match status {
    WorkflowStatus::Backlog => Icon::CircleDashed,
    WorkflowStatus::Todo => Icon::Circle,
    WorkflowStatus::InProgress => Icon::CircleDotDashed,
    WorkflowStatus::InReview => Icon::Eye,
    WorkflowStatus::Done => Icon::CircleCheck,
    WorkflowStatus::Canceled => Icon::CircleSlash,
  };
  let word = ctx.t(crate::ui::status::workflow_key(status));
  labelled("task-property-status", (glyph, PaletteColor::TextSecondary), &word)
}

fn priority(ctx: &Ctx, task: &TaskContext) -> Element {
  let (glyph, key, color) = priority_look(task.detail.task.organization().priority());
  labelled("task-property-priority", (glyph, color), &ctx.t(key))
}

/// What the task has now, beside what its latest run started with: the run's, *changed* after it and what the task has
/// in its tooltip when they differ.
fn captured(ctx: &mut Ctx, id: &str, (run, now): (Arc<str>, Arc<str>), look: Look) -> Element {
  if run == now {
    return value(id, &run, look).into();
  }
  let shown = Row::new()
    .id(id.to_owned())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Start)
    .child(value(&format!("{id}-value"), &run, look).flex(0.0).flex_shrink(1.0))
    .child(
      value(
        &format!("{id}-changed"),
        &ctx.t("summary.properties.changed"),
        Look::Muted,
      )
      .flex(0.0)
      .flex_shrink(0.0),
    );
  let tip = ctx.t_args("summary.properties.taskNow", [("value", now)]);
  Tooltip::new(format!("{id}-tip"), tip).wrap(ctx, shown)
}

/// The pipeline the latest run pinned, else what its first run would start with now.
fn pipeline(ctx: &mut Ctx, task: &TaskContext, tasks: Option<&ProjectTasks>) -> Element {
  let detail = &task.detail;
  let details = detail.task.details();
  let published = details
    .pipeline
    .and_then(|chosen| tasks?.pipelines.iter().find(|pipeline| pipeline.id == chosen).cloned());
  let now = match (&detail.missing_pipeline, published) {
    (Some(slug), _) => message_text(
      ctx,
      &PipelineLabel::Missing {
        slug: Arc::from(slug.as_str()),
      }
      .message(),
    ),
    (None, Some(pipeline)) => ctx.t_args(
      "summary.properties.pipelineValue",
      [
        ("pipeline", pipeline.name.to_string()),
        ("version", pipeline.version.to_string()),
      ],
    ),
    (None, None) => ctx.t("summary.properties.notChosen"),
  };
  let id = "task-property-pipeline";
  match run_setup(task) {
    Setup::None => value(id, &now, Look::Primary).into(),
    Setup::NotRecorded => value(id, &ctx.t("summary.properties.notRecorded"), Look::Muted).into(),
    Setup::Run(run) => {
      let version = run.version.to_string();
      let pinned = if run.pipeline.is_empty() {
        ctx.t_args("summary.properties.pipelineGone", [("version", version)])
      } else {
        ctx.t_args(
          "summary.properties.pipelineValue",
          [("pipeline", run.pipeline.to_string()), ("version", version)],
        )
      };
      captured(ctx, id, (pinned, now), Look::Primary)
    }
  }
}

/// What the properties show of the latest run's setup.
pub(super) enum Setup<'a> {
  /// No run yet: the task's own.
  None,
  /// The run cannot be read, or ran on another computer.
  NotRecorded,
  Run(&'a orchester_workspace::tasks::detail::RunView),
}

pub(super) fn run_setup(task: &TaskContext) -> Setup<'_> {
  let detail = &task.detail;
  match (detail.requirements.latest_run(), detail.run.as_ref()) {
    (LatestRun::Run(started), Some(run)) if started.run == run.id() => Setup::Run(run),
    (LatestRun::None, _) => Setup::None,
    _ => Setup::NotRecorded,
  }
}

/// The token and time limits the latest run started with, else the task's.
fn limits(ctx: &mut Ctx, task: &TaskContext) -> Element {
  let id = "task-property-limits";
  let limits = task.detail.task.details().limits;
  let now = limits_text(ctx, (limits.budget(), limits.max_duration()));
  match run_setup(task) {
    Setup::None => value(id, &now, Look::Secondary).into(),
    Setup::NotRecorded => value(id, &ctx.t("summary.properties.notRecorded"), Look::Muted).into(),
    Setup::Run(run) => {
      let created = run.run.created();
      let started = limits_text(ctx, (created.budget, created.max_duration));
      captured(ctx, id, (started, now), Look::Secondary)
    }
  }
}

/// "500k tokens · 90 min", either alone, or *No limits*.
fn limits_text(ctx: &Ctx, (budget, max_duration): (Option<TokenBudget>, Option<Duration>)) -> Arc<str> {
  let tokens = budget.map(|budget| ctx.t_args("summary.properties.tokens", [("budget", compact_tokens(budget.get()))]));
  let time = max_duration.map(|limit| {
    ctx.t_args(
      "summary.properties.minutes",
      [("minutes", whole_minutes(limit).to_string())],
    )
  });
  match (tokens, time) {
    (Some(tokens), Some(time)) => ctx.t_args("summary.properties.both", [("first", tokens), ("second", time)]),
    (Some(one), None) | (None, Some(one)) => one,
    (None, None) => ctx.t("summary.properties.noLimits"),
  }
}

/// The task's tags by name, once the project's tasks are read; *—* without any.
fn tags(ctx: &Ctx, task: &TaskContext, tasks: Option<&ProjectTasks>) -> Element {
  let organization = task.detail.task.organization();
  let names = tasks.map(|tasks| {
    tasks
      .tags
      .iter()
      .filter(|tag| organization.tags().contains(&tag.id()))
      .map(|tag| tag.name().as_str().to_owned())
      .collect()
  });
  listed(ctx, "task-property-tags", names)
}

/// The keys of the tasks the task waits for, once the project's tasks are read; *—* without any.
fn depends(ctx: &Ctx, task: &TaskContext, tasks: Option<&ProjectTasks>) -> Element {
  let organization = task.detail.task.organization();
  let keys = tasks.and_then(|tasks| {
    organization
      .dependencies()
      .iter()
      .map(|link| {
        Some(
          tasks
            .tasks
            .iter()
            .find(|task| task.id() == link.task)?
            .key()
            .to_string(),
        )
      })
      .collect::<Option<Vec<String>>>()
  });
  listed(ctx, "task-property-depends", keys)
}

fn listed(ctx: &Ctx, id: &str, names: Option<Vec<String>>) -> Element {
  match names {
    None => value(id, &ctx.t("summary.properties.notKnown"), Look::Muted).into(),
    Some(names) if names.is_empty() => value(id, &ctx.t("summary.properties.none"), Look::Muted).into(),
    Some(names) => value(id, &names.join(&ctx.t("summary.properties.separator")), Look::Secondary).into(),
  }
}

/// The branch of the latest run, on one line cut to its room with the whole name in a tooltip; *—* before one has its
/// own.
fn branch(ctx: &mut Ctx, task: &TaskContext) -> Element {
  let id = "task-property-branch";
  match task.detail.run.as_ref().and_then(|run| run.run.own_branch()) {
    Some(branch) => cut(ctx, id, branch.as_str()),
    None => value(id, &ctx.t("summary.properties.none"), Look::Muted).into(),
  }
}

/// An identifier on one line in Geist Mono 12 `text_secondary`, cut to its room with the whole of it in a tooltip.
pub fn cut(ctx: &mut Ctx, id: &str, text: &str) -> Element {
  let text = Truncated::new(id.to_owned(), text.to_owned(), TypographyStyle::Mono)
    .color(PaletteColor::TextSecondary)
    .render(ctx);
  Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(text).into()
}

/// *3 memory notes ↗* (`Memory link` `lJQcf`): the task's memory in the Summary's place.
fn memory_link(ctx: &mut Ctx, task: &TaskContext) -> Element {
  let words = match crate::screens::task::memory_count(ctx, task) {
    None => ctx.t("summary.properties.memory.reading"),
    Some(0) => ctx.t("summary.properties.memory.none"),
    Some(1) => ctx.t("summary.properties.memory.one"),
    Some(count) => ctx.t_args("summary.properties.memory.other", [("count", count.to_string())]),
  };
  let navigation = Navigation::of(ctx);
  let route = Route::Project(task.project.open.project.id(), task.page(TaskView::Memory));
  opening(navigation.to(route))
    .id("task-memory-link")
    .describe("a11y_name", words.clone())
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .child(icon(Icon::Brain, IconSize::Base, PaletteColor::TextMuted))
    .child(value("task-memory-link-label", &words, Look::Secondary))
    .child(icon(Icon::ArrowUpRight, IconSize::Sm, PaletteColor::TextMuted))
    .into()
}
