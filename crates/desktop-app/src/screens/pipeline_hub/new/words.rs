//! What a new pipeline's panel says of it: why *Create v1* is off, written beside the button as *Save*'s is (P14
//! `Rszkt`: *Name it and choose a profile for 02 Plan and 05 Review · nothing is saved yet*; P14a `d4f9Vi`: *Choose a
//! name with a free slug*; P14c `g09dL`: *Add a step first to Create v1*), or what it saves; its row's line in the list;
//! and the line over its name.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_workspace::pipelines::{
  local_edit::Unpublishable,
  ready::{Blocked, StepProblem},
};

use super::Choice;
use crate::{
  screens::pipeline_hub::{flow::number, model::Entry},
  ui::message::message_text,
};

/// How the name typed stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Named {
  /// Nothing is typed.
  Empty,
  /// What is typed has no letter or digit a slug is made of.
  NoSlug,
  /// Another pipeline has the slug it makes.
  Taken,
  Free,
}

/// What, besides the name, keeps the pipeline from being created.
pub(super) enum Lacks<'a> {
  /// *Copy a pipeline* is chosen with no pipeline to copy.
  Copy,
  /// What keeps its steps from being published, with the steps that do.
  Steps(&'a Unpublishable, &'a [StepProblem]),
}

/// Why *Create v1* is off: the name first, which alone keeps a copy off (P14a); `None` once it can be created.
pub(super) fn off(ctx: &Ctx, named: Named, lacks: Option<Lacks<'_>>) -> Option<Arc<str>> {
  match named {
    Named::Taken => return Some(ctx.t("pipeline_new.bar.slugTaken")),
    Named::NoSlug => return Some(ctx.t("pipeline_new.bar.noSlug")),
    Named::Empty | Named::Free => {}
  }
  let unnamed = named == Named::Empty;
  let Some(lacks) = lacks else {
    return unnamed.then(|| ctx.t("pipeline_new.bar.name"));
  };
  let (key, steps) = match lacks {
    Lacks::Copy => ("copy", None),
    Lacks::Steps(Unpublishable::NoSteps, _) => ("noSteps", None),
    Lacks::Steps(Unpublishable::Broken, _) => ("broken", None),
    Lacks::Steps(Unpublishable::Refused(why), _) => return Some(message_text(ctx, why)),
    Lacks::Steps(Unpublishable::Steps, problems) => (kind(problems), Some(steps(ctx, problems))),
  };
  let key = if unnamed {
    format!("pipeline_new.bar.named.{key}")
  } else {
    format!("pipeline_new.bar.{key}")
  };
  Some(match steps {
    Some(steps) => ctx.t_args(&key, [("steps", steps)]),
    None => ctx.t(&key),
  })
}

/// What *Create v1* saves, beside it once it can: *Saves Feature delivery copy as v1*.
pub(super) fn creates(ctx: &Ctx, name: &str) -> Arc<str> {
  ctx.t_args("pipeline_new.bar.creates", [("name", name.trim())])
}

/// Whether the steps lack a profile, a tool, or something else.
fn kind(problems: &[StepProblem]) -> &'static str {
  let profile = |step: &StepProblem| matches!(step.blocked, Blocked::NoProfile | Blocked::Gone { tool: false, .. });
  let tool = |step: &StepProblem| matches!(step.blocked, Blocked::NoTool | Blocked::Gone { tool: true, .. });
  if problems.iter().all(profile) {
    "profile"
  } else if problems.iter().all(tool) {
    "tool"
  } else {
    "ready"
  }
}

/// The steps by their number and name: one, two joined (*02 Plan and 05 Review*), or how many.
fn steps(ctx: &Ctx, problems: &[StepProblem]) -> String {
  let one = |step: &StepProblem| {
    ctx
      .t_args(
        "pipeline_new.steps.one",
        [("number", number(step.number)), ("step", step.name.to_string())],
      )
      .to_string()
  };
  match problems {
    [only] => one(only),
    [first, second] => ctx
      .t_args(
        "pipeline_new.steps.two",
        [("first", one(first)), ("second", one(second))],
      )
      .to_string(),
    many => ctx
      .t_args("pipeline_new.steps.many", [("count", many.len().to_string())])
      .to_string(),
  }
}

/// The row's line: where it starts from, how many steps it has, and that nothing is saved.
pub(super) fn line(ctx: &Ctx, choice: Choice, source: Option<&Entry>, steps: usize) -> Arc<str> {
  let count = [("steps", steps.to_string())];
  match (choice, source) {
    (Choice::Copy, Some(source)) => ctx.t_args("pipeline_new.line.copy", [("name", source.name.to_string())]),
    (Choice::Copy, None) => ctx.t("pipeline_new.line.copyNone"),
    (Choice::Starter, _) => ctx.t_args("pipeline_new.line.starter", count),
    (Choice::Empty, _) if steps == 0 => ctx.t("pipeline_new.line.empty"),
    (Choice::Empty, _) if steps == 1 => ctx.t("pipeline_new.line.emptyOne"),
    (Choice::Empty, _) => ctx.t_args("pipeline_new.line.emptySteps", count),
  }
}

/// What the line over the name says: that it is new, and what it copies.
pub(super) fn what(ctx: &Ctx, source: Option<&Entry>) -> String {
  let copied = source.and_then(|source| source.number.map(|version| (source, version)));
  match copied {
    Some((source, version)) => ctx.t_args(
      "pipeline_new.whatCopy",
      [("name", source.name.to_string()), ("version", version.to_string())],
    ),
    None => ctx.t("pipeline_new.what"),
  }
  .to_string()
}
