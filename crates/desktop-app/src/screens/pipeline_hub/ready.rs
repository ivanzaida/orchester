//! *Ready?* under the pipeline's flow (`Ready?` `j0OZhl` in `Pipelines · v2` P1 `s7LMyQ`, `eA5ed` in P8 `vujkk`): *all
//! set* in `success` with one summary row, or *N missing* in `warning` with one row per problem and what to do about it
//! at its end (*Choose →* opens the step, *Open →* the profile or tool in the Agents hub, *Add step →* adds a step as
//! the head's *Add step* does, *Edit steps →* opens the editor as the flow's notice does, *Save →* saves as the bar's
//! *Save* does). What is missing is the workspace's ([`orchester_workspace::pipelines::ready`]): what is wrong with the
//! draft, the steps whose card stands in amber (also when the steps cannot be drawn as one line), and what the
//! readiness read says this computer lacks. The rows are the Agents hub's
//! ([`crate::screens::team_hub::profile::ready`]).
//!
//! The panel draws a pipeline's draft when it has one, so *Ready?* describes the draft; over a published version it
//! says so in its title, as the list and the chips speak of the version tasks use. What the readiness read says is
//! about that version: a profile or tool the draft no longer runs is left out, and a row about the version's profiles
//! and tools as a whole names the version.

use std::{collections::BTreeSet, sync::Arc};

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{PipelineEdit, ResourceKind, StepKey, VersionNumber},
};
use orchester_workspace::pipelines::{
  flow::{FlowRead, PipelineFlow, Runs},
  ready::{Basis, Blocked, Draft, Problem, StepProblem, UndecidedMember, problems},
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
  /// Add a step at the end of the line, as the head's *Add step* does.
  AddStep,
  /// Open the editor, where the steps are put right.
  EditSteps,
  /// Open the Agents hub, where connections, programs and values are set.
  Agents,
  /// Save, as the bar's *Save* does: it publishes a draft that is only not published yet.
  Save,
}

impl Offer for Fix {
  fn link_key(&self) -> &'static str {
    match self {
      Self::Choose(_) => "pipeline_hub.ready.choose",
      Self::Resource(_) | Self::Agents => "pipeline_hub.ready.open",
      Self::AddStep => "pipeline_hub.ready.addStep",
      Self::EditSteps => "pipeline_hub.ready.editSteps",
      Self::Save => "pipeline_hub.ready.save",
    }
  }

  fn name(&self) -> &'static str {
    match self {
      Self::Choose(_) => "choose",
      Self::Resource(_) => "open",
      Self::AddStep => "add-step",
      Self::EditSteps => "edit-steps",
      Self::Agents => "agents",
      Self::Save => "save",
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
  /// What the bar's *Save* does now, which *Save →* does too; `None` while it cannot save.
  pub save: Option<&'a Press>,
}

/// The group for `shown.entry`, which is none for an archived pipeline, which has no steps.
pub(super) fn group(ctx: &mut Ctx, shown: &Shown<'_>) -> Option<Element> {
  let Shown { entry, read, .. } = shown;
  if entry.archived.is_some() {
    return None;
  }
  let over = over_version(entry, read);
  let runs = over.map(|_| runs_of(shown.current));
  let found = problems(Basis {
    flow: read,
    standing: entry.standing.as_ref(),
    undecided: &entry.undecided,
    published: entry.number.is_some(),
    runs: runs.as_deref(),
  });
  let parts = if found.is_empty() {
    vec![summary(ctx, read.flow.as_ref().ok())]
  } else {
    found
      .iter()
      .enumerate()
      .map(|(index, problem)| part(ctx, problem, (index, over), shown.save.is_some()))
      .collect()
  };
  let navigation = Navigation::of(ctx);
  let new_step = new_step_name(ctx);
  let press = |fix: &Fix| press(shown, (&navigation, &new_step), fix);
  let title = if over.is_some() {
    ctx.t("pipeline_hub.ready.titleDraft")
  } else {
    ctx.t("pipeline_hub.ready.title")
  };
  let busy = if shown.locked { Busy::Taking } else { Busy::Idle };
  Some(section(ctx, (&title, &parts), found.len(), (&press, busy)).into())
}

/// Whether the pipeline as the panel draws it can run once saved: nothing is missing but, for a pipeline never
/// published, that it is not published yet, which a save does.
pub(super) fn can_run(entry: &Entry, read: &FlowRead) -> bool {
  problems(Basis {
    flow: read,
    standing: entry.standing.as_ref(),
    undecided: &entry.undecided,
    published: entry.number.is_some(),
    runs: None,
  })
  .iter()
  .all(|problem| *problem == Problem::Draft(Draft::NotPublished))
}

/// The published version a draft the panel draws stands over, whose readiness the rows read; `None` when the panel
/// draws the version itself or the pipeline was never published.
fn over_version(entry: &Entry, read: &FlowRead) -> Option<VersionNumber> {
  entry.number.filter(|_| read.draft)
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
    Fix::Save => {
      return shown
        .save
        .cloned()
        .expect("*Save →* is offered only while Save can save");
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
/// names it, as the draft may run others.
/// `can_save`: *Save* can save now, so a draft that is only not published offers it.
fn part(ctx: &Ctx, problem: &Problem, (index, over): (usize, Option<VersionNumber>), can_save: bool) -> Part<Fix> {
  match problem {
    Problem::Draft(draft) => labelled(
      "draft",
      ctx.t("pipeline_hub.ready.draft"),
      draft_words(ctx, *draft),
      draft_fix(*draft, can_save),
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
/// is about the published version `over` a draft.
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

/// What is wrong with the draft as a whole.
fn draft_words(ctx: &Ctx, draft: Draft) -> Arc<str> {
  ctx.t(match draft {
    Draft::NoSteps => "pipeline_hub.ready.noSteps",
    Draft::Broken => "pipeline_hub.flow.broken.title",
    Draft::Branches => "pipeline_hub.flow.branches.title",
    Draft::NotPublished => "pipeline_hub.ready.notPublished",
  })
}

/// *Add step →* for a draft with none, *Edit steps →* for steps the editor puts right, *Save →* for a draft that is
/// only not published yet, while *Save* can save.
const fn draft_fix(draft: Draft, can_save: bool) -> Option<Fix> {
  match draft {
    Draft::NoSteps => Some(Fix::AddStep),
    Draft::Broken | Draft::Branches => Some(Fix::EditSteps),
    Draft::NotPublished if can_save => Some(Fix::Save),
    Draft::NotPublished => None,
  }
}

/// What is wrong with a step, short enough for a row's value: *No agent profile*.
fn step_words(ctx: &Ctx, blocked: &Blocked) -> Arc<str> {
  ctx.t_args(
    match blocked {
      Blocked::NoProfile => "pipeline_hub.ready.noProfile",
      Blocked::NoTool => "pipeline_hub.ready.noTool",
      Blocked::ProfileNotPublished { .. } => "pipeline_hub.flow.profileNotPublished",
      Blocked::ProfileConnectionGone { .. } => "pipeline_hub.ready.profileConnectionGone",
      Blocked::ToolNotPublished { .. } => "pipeline_hub.flow.toolNotPublished",
      Blocked::ToolNotRunnable { .. } => "pipeline_hub.ready.toolNotRunnable",
    },
    [("tool", named(blocked)), ("profile", named(blocked))],
  )
}

/// *Choose →* for a step with nothing to run, *Open →* for one that names a profile or tool that cannot run.
fn step_fix(step: &StepProblem) -> Fix {
  match &step.blocked {
    Blocked::NoProfile | Blocked::NoTool => Fix::Choose(step.key.clone()),
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
