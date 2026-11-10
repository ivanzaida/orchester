//! *Ready?* under the pipeline's flow (`Ready?` `j0OZhl` in `Pipelines · v2` P1 `s7LMyQ`, `eA5ed` in P8 `vujkk`): *all
//! set* in `success` with one summary row, or *N missing* in `warning` with one row per problem and what to do about it
//! at its end (*Choose →* opens the step, *Open →* the profile or tool in the Agents hub, *Edit steps →* the editor).
//! What is missing is the workspace's ([`orchester_workspace::pipelines::ready`]): what is wrong with the draft, the
//! steps whose card stands in amber, and what the readiness read says this computer lacks. The rows are the Agents
//! hub's ([`crate::screens::team_hub::profile::ready`]).
//!
//! The panel draws a pipeline's draft when it has one, so *Ready?* describes the draft; over a published version it
//! says so in its title, as the list and the chips speak of the version tasks use.

use std::{collections::BTreeSet, sync::Arc};

use lurq::app::ctx::Ctx;
use orchester_core::resource::{ResourceKind, StepKey};
use orchester_workspace::pipelines::{
  flow::{FlowRead, PipelineFlow, Runs},
  ready::{Basis, Draft, Problem, StepProblem, UndecidedMember, problems},
};

use super::{
  flow::{self, number},
  model::Entry,
  step_line::tool_name,
  undecided::{member_words, undecided_words},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{HubView, PipelineView, ProjectPage, Route},
  screens::{
    local_hub::words::missing,
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
  /// Open the profile or tool, where it is published or its connection is chosen.
  Resource(flow::Fix),
  /// Open the editor, where the first step is added.
  AddStep,
  /// Open the editor, where the steps are put right.
  EditSteps,
  /// Open the Agents hub, where connections, programs and values are set.
  Agents,
}

impl Offer for Fix {
  fn link_key(&self) -> &'static str {
    match self {
      Self::Choose(_) => "pipeline_hub.ready.choose",
      Self::Resource(_) | Self::Agents => "pipeline_hub.ready.open",
      Self::AddStep => "pipeline_hub.ready.addStep",
      Self::EditSteps => "pipeline_hub.ready.editSteps",
    }
  }

  fn name(&self) -> &'static str {
    match self {
      Self::Choose(_) => "choose",
      Self::Resource(_) => "open",
      Self::AddStep => "add-step",
      Self::EditSteps => "edit-steps",
      Self::Agents => "agents",
    }
  }
}

/// The group for `entry`, which is none for an archived pipeline, which has no steps. `read` is the flow the panel draws:
/// the edit's while the panel holds one, so what is missing is what the person sees and saves; only what this computer
/// lacks (`entry.standing`) is about the version tasks use, which an edit does not change until it is saved.
pub(super) fn group(
  ctx: &mut Ctx,
  project: &ShownProject,
  entry: &Entry,
  read: &FlowRead,
) -> Option<lurq::node::Element> {
  if entry.archived.is_some() {
    return None;
  }
  let found = problems(Basis {
    flow: read,
    standing: entry.standing.as_ref(),
    undecided: &entry.undecided,
    published: entry.number.is_some(),
  });
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
      Fix::Resource(resource) => resource.page(),
      Fix::AddStep | Fix::EditSteps => ProjectPage::Pipeline(entry.id, PipelineView::default()),
      Fix::Agents => ProjectPage::AgentsHub(HubView::default()),
    };
    Press::Open(navigation.to(Route::Project(project.open.project.id(), page)))
  };
  let title = if read.draft && entry.number.is_some() {
    ctx.t("pipeline_hub.ready.titleDraft")
  } else {
    ctx.t("pipeline_hub.ready.title")
  };
  Some(section(ctx, (&title, &parts), found.len(), (&press, Busy::Idle)).into())
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
    Problem::Draft(draft) => about(
      ctx,
      ("draft", "pipeline_hub.ready.draft"),
      draft_words(ctx, *draft),
      draft_fix(*draft),
    ),
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
    Problem::Member(member) => Part {
      id: format!("member-{}", member.id),
      label: member.name.clone(),
      state: State::Missing {
        words: member_words(ctx, &member.why),
        fix: Some(Fix::Resource(resource_fix(member))),
        blocking: false,
      },
    },
    Problem::Missing(part) => about_runs(
      ctx,
      &format!("missing-{index}"),
      missing(ctx, ResourceKind::Pipeline, part),
      Some(Fix::Agents),
    ),
    Problem::Undecided(why) => about_runs(ctx, "undecided", undecided_words(ctx, why, &[]), None),
  }
}

/// A row about what the steps run as a whole, not about one step.
fn about_runs(ctx: &Ctx, id: &str, words: Arc<str>, fix: Option<Fix>) -> Part<Fix> {
  about(ctx, (id, "pipeline_hub.ready.profilesAndTools"), words, fix)
}

fn about(ctx: &Ctx, (id, label): (&str, &str), words: Arc<str>, fix: Option<Fix>) -> Part<Fix> {
  Part {
    id: id.to_owned(),
    label: ctx.t(label),
    state: State::Missing {
      words,
      fix,
      blocking: false,
    },
  }
}

/// What is wrong with the draft as a whole.
fn draft_words(ctx: &Ctx, draft: Draft) -> Arc<str> {
  ctx.t(match draft {
    Draft::NoSteps => "pipeline_hub.ready.noSteps",
    Draft::Broken => "pipeline_hub.flow.broken.title",
    Draft::Branches => "pipeline_hub.flow.branches.title",
    Draft::NotPublished => "pipeline_hub.ready.notPublished",
  })
}

/// *Add step →* for a draft with none, *Edit steps →* for steps the editor puts right; a draft that is only not
/// published yet has nothing to fix here.
const fn draft_fix(draft: Draft) -> Option<Fix> {
  match draft {
    Draft::NoSteps => Some(Fix::AddStep),
    Draft::Broken | Draft::Branches => Some(Fix::EditSteps),
    Draft::NotPublished => None,
  }
}

/// What is wrong with a step, short enough for a row's value: *No agent profile*.
fn step_words(ctx: &Ctx, step: &StepProblem) -> Arc<str> {
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

/// *Choose →* for a step with nothing to run, *Open →* for one that names a profile or tool that cannot run.
fn step_fix(step: &StepProblem) -> Option<Fix> {
  if step.runs.is_unassigned() {
    Some(Fix::Choose(step.key.clone()))
  } else {
    flow::Fix::of(&step.runs).map(Fix::Resource)
  }
}

/// A profile or tool that cannot be told is opened in the Agents hub, where its reason is shown and fixed.
fn resource_fix(member: &UndecidedMember) -> flow::Fix {
  if member.kind == ResourceKind::AgentProfile {
    flow::Fix::Profile(member.id)
  } else {
    flow::Fix::Tool(member.id)
  }
}
