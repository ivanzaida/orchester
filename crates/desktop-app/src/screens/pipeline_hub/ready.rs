//! *Ready?* under the pipeline's flow (`Ready?` `j0OZhl` in `Pipelines · v2` P1 `s7LMyQ`, `eA5ed` in P8 `vujkk`): *all
//! set* in `success` with one summary row, or *N missing* in `warning` with one row per problem and what to do about it
//! at its end (*Choose →* opens the step, *Open →* the profile or tool in the Agents hub, *Add step →* adds a step as
//! the head's *Add step* does, *Edit steps →* opens the editor as the flow's notice does). What is missing is the
//! workspace's ([`orchester_workspace::pipelines::ready`]): what is wrong with the steps as a whole, the steps whose
//! card stands in amber (also when the steps cannot be drawn as one line), and what the readiness read says this
//! computer lacks. The rows are the Agents hub's ([`crate::screens::team_hub::profile::ready`]).
//!
//! The panel draws the edit it holds when it holds one, so *Ready?* reads the edit as it stands; while *Save* is off
//! for it, its count says so (*1 missing · Save stays off*, P8). What the readiness read says is about the published
//! version: a profile or tool the edit no longer runs is left out, and a row about the version's profiles and tools as
//! a whole names the version.

use std::{collections::BTreeSet, sync::Arc};

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{PipelineEdit, ResourceKind, StepKey, VersionNumber},
};
use orchester_workspace::pipelines::{
  flow::{FlowRead, PipelineFlow, Runs},
  ready::{Basis, Blocked, Problem, StepProblem, UndecidedMember, Whole, problems},
};

use super::{
  edit::Edit,
  flow::{self, number},
  model::Entry,
  step_line::named,
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
        ready::{counted, section_saying},
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
  /// Open the step whose profile or tool is gone, with the Agent picker open to choose another.
  ChooseAnother(StepKey),
  /// Open the profile or tool, where it is published or its connection is chosen.
  Resource(flow::Fix),
  /// Add a step at the end of the line, as the head's *Add step* does.
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
      Self::ChooseAnother(_) => "pipeline_hub.ready.chooseAnother",
      Self::Resource(_) | Self::Agents => "pipeline_hub.ready.open",
      Self::AddStep => "pipeline_hub.ready.addStep",
      Self::EditSteps => "pipeline_hub.ready.editSteps",
    }
  }

  fn name(&self) -> &'static str {
    match self {
      Self::Choose(_) => "choose",
      Self::ChooseAnother(_) => "choose-another",
      Self::Resource(_) => "open",
      Self::AddStep => "add-step",
      Self::EditSteps => "edit-steps",
      Self::Agents => "agents",
    }
  }
}

/// What *Ready?* is drawn from.
pub(super) struct Shown<'a> {
  pub project: &'a ShownProject,
  pub entry: &'a Entry,
  /// The flow the panel draws: the edit's while the panel holds one, so what is missing is what the person sees and
  /// saves; only what this computer lacks (`entry.standing`) is about the version tasks use, which an edit does not
  /// change until it is saved.
  pub read: &'a FlowRead,
  /// What the panel shows, which *Add step →* adds to.
  pub current: &'a PipelineEdit,
  pub edit: &'a Edit,
  /// A save is under way or asks: nothing is changed now.
  pub locked: bool,
  /// *Save* is off: the edit cannot be published as it stands, which the count says after what is missing.
  pub save_off: bool,
  /// Set by *Choose another →*: the step it opens opens its Agent picker.
  pub pick_agent: &'a Signal<bool>,
}

/// The group for `shown.entry`, which is none for an archived pipeline, which has no steps.
pub(super) fn group(ctx: &mut Ctx, shown: &Shown<'_>) -> Option<Element> {
  let Shown { entry, read, .. } = shown;
  if entry.archived.is_some() {
    return None;
  }
  let over = entry.number.filter(|_| shown.edit.holds());
  let runs = over.map(|_| runs_of(shown.current));
  let found = problems(Basis {
    flow: read,
    standing: entry.standing.as_ref(),
    undecided: &entry.undecided,
    runs: runs.as_deref(),
  });
  let parts = if found.is_empty() {
    vec![summary(ctx, read.flow.as_ref().ok())]
  } else {
    found
      .iter()
      .enumerate()
      .map(|(index, problem)| part(ctx, problem, (index, over)))
      .collect()
  };
  let navigation = Navigation::of(ctx);
  let new_step = new_step_name(ctx);
  let press = |fix: &Fix| press(shown, (&navigation, &new_step), fix);
  let title = ctx.t("pipeline_hub.ready.title");
  let busy = if shown.locked { Busy::Taking } else { Busy::Idle };
  let (words, color) = counted(ctx, &parts, found.len());
  // *1 missing · Save stays off* (P8 `vujkk`): what is missing keeps the edit from being stored.
  let words = if shown.save_off && !found.is_empty() {
    ctx.t_args(
      "pipeline_hub.ready.missingSaveOff",
      [("count", found.len().to_string())],
    )
  } else {
    words
  };
  Some(section_saying(ctx, (&title, &parts), (words, color), (&press, busy)).into())
}

/// Whether the pipeline as the panel draws it can run once saved: nothing is missing.
pub(super) fn can_run(entry: &Entry, read: &FlowRead) -> bool {
  problems(Basis {
    flow: read,
    standing: entry.standing.as_ref(),
    undecided: &entry.undecided,
    runs: None,
  })
  .is_empty()
}

/// The profiles and tools `current`'s steps run, and its integrator.
fn runs_of(current: &PipelineEdit) -> Vec<ResourceId> {
  let integrator = current.deliver().and_then(|deliver| deliver.integrator());
  current
    .steps()
    .iter()
    .filter_map(|step| step.kind.executor())
    .chain(integrator)
    .collect()
}

/// What pressing `fix` does.
fn press(shown: &Shown<'_>, (navigation, new_step): (&Navigation, &Name), fix: &Fix) -> Press {
  let id = shown.entry.id;
  let page = match fix {
    Fix::AddStep => {
      let (edit, name) = (shown.edit.clone(), new_step.clone());
      return Press::run(move || {
        edit.change(|held| {
          held.add_step(name.clone(), None);
        });
      });
    }
    Fix::ChooseAnother(key) => {
      let (navigation, pick) = (navigation.clone(), shown.pick_agent.clone());
      let route = Route::Project(
        shown.project.open.project.id(),
        ProjectPage::Pipeline(id, PipelineView::Step(key.clone())),
      );
      return Press::run(move || {
        pick.set(true);
        navigation.go(route.clone());
      });
    }
    Fix::Choose(key) => ProjectPage::Pipeline(id, PipelineView::Step(key.clone())),
    Fix::Resource(resource) => resource.page(),
    Fix::EditSteps => ProjectPage::Pipeline(id, PipelineView::default()),
    Fix::Agents => ProjectPage::AgentsHub(HubView::default()),
  };
  Press::Open(navigation.to(Route::Project(shown.project.open.project.id(), page)))
}

/// The name a step added by *Add step →* starts with: the head's *Add step*'s.
fn new_step_name(ctx: &Ctx) -> Name {
  Name::new(&ctx.t("pipeline_hub.flow.move.newStep")).expect("the translated name of a new step is a valid name")
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

/// The row of `problem`. Over a published version (`over`), a row about that version's profiles and tools as a whole
/// names it, as the edit may run others.
fn part(ctx: &Ctx, problem: &Problem, (index, over): (usize, Option<VersionNumber>)) -> Part<Fix> {
  match problem {
    Problem::Whole(whole) => labelled(
      "steps",
      ctx.t("pipeline_hub.panel.steps"),
      whole_words(ctx, *whole),
      Some(whole_fix(*whole)),
    ),
    Problem::Step(step) => labelled(
      &format!("step-{}", step.key),
      ctx.t_args(
        "pipeline_hub.ready.step",
        [("number", number(step.number)), ("name", step.name.to_string())],
      ),
      step_words(ctx, &step.blocked),
      Some(step_fix(step)),
    ),
    Problem::Member(member) => labelled(
      &format!("member-{}", member.id),
      member.name.clone(),
      member_words(ctx, &member.why),
      Some(Fix::Resource(resource_fix(member))),
    ),
    Problem::Missing(part) => labelled(
      &format!("missing-{index}"),
      runs_label(ctx, over),
      missing(ctx, ResourceKind::Pipeline, part),
      Some(Fix::Agents),
    ),
    Problem::Undecided(why) => labelled("undecided", runs_label(ctx, over), undecided_words(ctx, why, &[]), None),
  }
}

/// The label of a row about what the steps run as a whole: *Profiles and tools*, or *Profiles and tools · v3* when it
/// is about the published version under the edit the panel holds (`over`).
fn runs_label(ctx: &Ctx, over: Option<VersionNumber>) -> Arc<str> {
  over.map_or_else(
    || ctx.t("pipeline_hub.ready.profilesAndTools"),
    |version| {
      ctx.t_args(
        "pipeline_hub.ready.profilesAndToolsOf",
        [("version", version.to_string())],
      )
    },
  )
}

fn labelled(id: &str, label: Arc<str>, words: Arc<str>, fix: Option<Fix>) -> Part<Fix> {
  Part {
    id: id.to_owned(),
    label,
    state: State::Missing {
      words,
      fix,
      blocking: false,
    },
  }
}

/// What is wrong with the steps as a whole.
pub(super) fn whole_words(ctx: &Ctx, whole: Whole) -> Arc<str> {
  ctx.t(match whole {
    Whole::NoSteps => "pipeline_hub.ready.noSteps",
    Whole::Broken => "pipeline_hub.flow.broken.title",
    Whole::Branches => "pipeline_hub.flow.branches.title",
  })
}

/// *Add step →* for steps with none, *Edit steps →* for steps the editor puts right.
const fn whole_fix(whole: Whole) -> Fix {
  match whole {
    Whole::NoSteps => Fix::AddStep,
    Whole::Broken | Whole::Branches => Fix::EditSteps,
  }
}

/// What is wrong with a step, short enough for a row's value: *No agent profile*.
fn step_words(ctx: &Ctx, blocked: &Blocked) -> Arc<str> {
  ctx.t_args(
    match blocked {
      Blocked::NoProfile => "pipeline_hub.ready.noProfile",
      Blocked::NoTool => "pipeline_hub.ready.noTool",
      Blocked::ProfileNotPublished { .. } => "pipeline_hub.flow.profileNotPublished",
      Blocked::Gone { tool: false, .. } => "pipeline_hub.stepFlow.profileGone",
      Blocked::Gone { tool: true, .. } => "pipeline_hub.stepFlow.toolGone",
      Blocked::ProfileConnectionGone { .. } => "pipeline_hub.ready.profileConnectionGone",
      Blocked::ToolNotPublished { .. } => "pipeline_hub.flow.toolNotPublished",
      Blocked::ToolNotRunnable { .. } => "pipeline_hub.ready.toolNotRunnable",
    },
    [("tool", named(blocked)), ("profile", named(blocked))],
  )
}

/// *Choose →* for a step with nothing to run, *Choose another →* for one whose profile or tool is gone (it has no page
/// to open), *Open →* for one that names a profile or tool that cannot run.
fn step_fix(step: &StepProblem) -> Fix {
  match &step.blocked {
    Blocked::NoProfile | Blocked::NoTool => Fix::Choose(step.key.clone()),
    Blocked::Gone { .. } => Fix::ChooseAnother(step.key.clone()),
    Blocked::ProfileNotPublished { id } | Blocked::ProfileConnectionGone { id, .. } => {
      Fix::Resource(flow::Fix::Profile(*id))
    }
    Blocked::ToolNotPublished { id } | Blocked::ToolNotRunnable { id, .. } => Fix::Resource(flow::Fix::Tool(*id)),
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
