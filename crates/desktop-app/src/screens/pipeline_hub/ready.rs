//! *Ready?* under the pipeline's flow (`Ready?` `j0OZhl` in `Pipelines · v2` P1 `s7LMyQ`, `eA5ed` in P8 `vujkk`): *all
//! set* in `success` with one summary row, or *N missing* in `warning` with one row per problem and what to do about it
//! at its end (*Choose →* opens the step, *Open →* the profile or tool in the Agents hub). What is missing is the
//! workspace's ([`orchester_workspace::pipelines::ready`]): the steps whose card stands in amber, and what the
//! readiness read says this computer lacks. The rows are the Agents hub's
//! ([`crate::screens::team_hub::profile::ready`]).

use std::{collections::BTreeSet, sync::Arc};

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::resource::{ResourceKind, StepKey};
use orchester_workspace::{
  pipelines::{
    flow::{PipelineFlow, Runs},
    ready::{Problem, StepProblem, problems, step_problems},
  },
  readiness::Undecided,
};

use super::{
  flow::{Fix as Open, number},
  model::Entry,
  words::Said,
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{HubView, PipelineView, ProjectPage, Route},
  screens::{
    local_hub::words::{missing, undecided},
    team_hub::{
      actions::Busy,
      profile::{
        fix::Offer,
        parts::{Part, State},
        ready::section,
      },
    },
  },
  ui::pressable::Press,
};

/// What a row of *Ready?* offers at its end.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Fix {
  /// Open the step that has nothing to run, where its profile or tool is chosen.
  Choose(StepKey),
  /// Open the profile or tool the step names, where it is published.
  Open(Open),
  /// Open the Agents hub, where connections, programs and values are set.
  Agents,
}

impl Offer for Fix {
  fn link_key(&self) -> &'static str {
    match self {
      Self::Choose(_) => "pipeline_hub.ready.choose",
      Self::Open(_) | Self::Agents => "pipeline_hub.ready.open",
    }
  }

  fn name(&self) -> &'static str {
    match self {
      Self::Choose(_) => "choose",
      Self::Open(_) => "open",
      Self::Agents => "agents",
    }
  }
}

/// The group for `entry`, which is none for an archived pipeline, which has no steps.
pub(super) fn group(ctx: &mut Ctx, project: &ShownProject, entry: &Entry) -> Option<lurq::node::Element> {
  let (Some(read), None) = (&entry.flow, entry.archived) else {
    return None;
  };
  let found = problems(read, entry.standing.as_ref());
  let parts = if found.is_empty() {
    vec![summary(ctx, read.flow.as_ref().ok())]
  } else {
    found
      .iter()
      .enumerate()
      .map(|(index, problem)| part(ctx, problem, index))
      .collect()
  };
  let navigation = Navigation::of(ctx);
  let press = |fix: &Fix| {
    let page = match fix {
      Fix::Choose(key) => ProjectPage::Pipeline(entry.id, PipelineView::Step(key.clone())),
      Fix::Open(open) => open.page(),
      Fix::Agents => ProjectPage::AgentsHub(HubView::default()),
    };
    Press::Open(navigation.to(Route::Project(project.open.project.id(), page)))
  };
  Some(
    section(
      ctx,
      (&ctx.t("pipeline_hub.ready.title"), &parts),
      found.len(),
      (&press, Busy::Idle),
    )
    .into(),
  )
}

/// The one row of a pipeline with nothing missing: the profiles and tools its steps run, which are ready.
fn summary(ctx: &Ctx, flow: Option<&PipelineFlow>) -> Part<Fix> {
  let names: BTreeSet<&str> = flow
    .into_iter()
    .flat_map(|flow| flow.stages.iter().flat_map(|stage| &stage.cards))
    .filter_map(|card| match &card.runs {
      Runs::Profile { name, .. } | Runs::Tool { name } => Some(name.as_ref()),
      _ => None,
    })
    .collect();
  let words = if names.is_empty() {
    ctx.t("pipeline_hub.ready.none")
  } else {
    ctx.t_args(
      "pipeline_hub.ready.allReady",
      [("names", names.into_iter().collect::<Vec<_>>().join(", "))],
    )
  };
  Part {
    id: "summary".to_owned(),
    label: ctx.t("pipeline_hub.ready.profilesAndTools"),
    state: State::Ready(words),
  }
}

fn part(ctx: &Ctx, problem: &Problem, index: usize) -> Part<Fix> {
  match problem {
    Problem::Step(step) => Part {
      id: format!("step-{}", step.key),
      label: ctx.t_args(
        "pipeline_hub.ready.step",
        [("number", number(step.number)), ("name", step.name.to_string())],
      ),
      state: State::Missing {
        words: step_words(ctx, step),
        fix: step_fix(step),
        blocking: false,
      },
    },
    Problem::Missing(part) => about_runs(
      ctx,
      format!("missing-{index}"),
      missing(ctx, ResourceKind::Pipeline, part),
      Some(Fix::Agents),
    ),
    Problem::Undecided(why) => about_runs(
      ctx,
      "undecided".to_owned(),
      undecided_words(ctx, why),
      undecided_fix(why),
    ),
  }
}

/// A row about what the steps run as a whole, not about one step.
fn about_runs(ctx: &Ctx, id: String, words: Arc<str>, fix: Option<Fix>) -> Part<Fix> {
  Part {
    id,
    label: ctx.t("pipeline_hub.ready.profilesAndTools"),
    state: State::Missing {
      words,
      fix,
      blocking: false,
    },
  }
}

/// What is wrong with a step, short enough for a row's value: *No agent profile*.
pub(super) fn step_words(ctx: &Ctx, step: &StepProblem) -> Arc<str> {
  ctx.t_args(
    match &step.runs {
      Runs::NoProfile => "pipeline_hub.ready.noProfile",
      Runs::NoTool => "pipeline_hub.ready.noTool",
      Runs::ProfileNotPublished { .. } => "pipeline_hub.flow.profileNotPublished",
      Runs::ToolNotPublished { .. } => "pipeline_hub.flow.toolNotPublished",
      Runs::ToolNotRunnable { .. } => "pipeline_hub.ready.toolNotRunnable",
      Runs::Profile { .. } | Runs::Tool { .. } => "pipeline_hub.ready.ready",
    },
    [("tool", tool_name(&step.runs))],
  )
}

fn tool_name(runs: &Runs) -> &str {
  match runs {
    Runs::ToolNotRunnable { name, .. } => name,
    _ => "",
  }
}

/// *Choose →* for a step with nothing to run, *Open →* for one that names a profile or tool that cannot run.
fn step_fix(step: &StepProblem) -> Option<Fix> {
  if step.runs.is_unassigned() {
    Some(Fix::Choose(step.key.clone()))
  } else {
    Open::of(&step.runs).map(Fix::Open)
  }
}

/// Why a pipeline cannot be told to be ready, as far as the person can act on it: a profile its steps run that has no
/// connection, or an archived resource it names; otherwise that nobody can tell yet.
pub(super) fn undecided_words(ctx: &Ctx, why: &Undecided) -> Arc<str> {
  match why {
    Undecided::Member { why, .. } if matches!(**why, Undecided::ConnectionGone(_)) => {
      ctx.t("pipeline_hub.ready.connectionGone")
    }
    other => undecided(ctx, other),
  }
}

/// A profile that lost its connection is fixed in the Agents hub, on the profile.
fn undecided_fix(why: &Undecided) -> Option<Fix> {
  match why {
    Undecided::Member { id, why } if matches!(**why, Undecided::ConnectionGone(_)) => {
      Some(Fix::Open(Open::Profile(*id)))
    }
    _ => None,
  }
}

/// The row's line of a draft some steps of which cannot run: the one step and what it lacks, or how many.
pub(super) fn line(ctx: &Ctx, entry: &Entry) -> Option<Said> {
  if entry.archived.is_some() || !entry.is_draft() {
    return None;
  }
  let steps = step_problems(entry.flow.as_ref()?);
  let text = match steps.as_slice() {
    [] => return None,
    [only] => ctx.t_args(
      step_line_key(&only.runs),
      [
        ("number", number(only.number)),
        ("step", only.name.to_string()),
        ("tool", tool_name(&only.runs).to_owned()),
      ],
    ),
    many => ctx.t_args("pipeline_hub.line.stepsNotReady", [("count", many.len().to_string())]),
  };
  Some(Said {
    text,
    color: PaletteColor::Warning,
  })
}

fn step_line_key(runs: &Runs) -> &'static str {
  match runs {
    Runs::NoProfile => "pipeline_hub.line.noProfile",
    Runs::NoTool => "pipeline_hub.line.noTool",
    Runs::ProfileNotPublished { .. } => "pipeline_hub.line.profileNotPublished",
    Runs::ToolNotPublished { .. } => "pipeline_hub.line.toolNotPublished",
    Runs::ToolNotRunnable { .. } => "pipeline_hub.line.toolNotRunnable",
    Runs::Profile { .. } | Runs::Tool { .. } => "pipeline_hub.line.stepsNotReady",
  }
}

/// How many steps cannot run, for the chip next to *draft*; none for an archived pipeline.
pub(super) fn steps_not_ready(entry: &Entry) -> usize {
  match (&entry.flow, entry.archived) {
    (Some(read), None) => step_problems(read).len(),
    _ => 0,
  }
}
