//! A pipeline as the Pipelines page draws its steps (`Pipelines · v2` P1 `s7LMyQ`, P8 `vujkk`): the steps in the
//! order they run in, the steps that run side by side in one stage, the approval gates on the line after their step,
//! the failure routes by the numbers the cards carry, and the Deliver step numbered last. The layout is derived from
//! the steps' `depends_on` ([`orchester_core::resource::flow_units`]); nothing of it is stored.

use std::{collections::HashMap, sync::Arc};

use orchester_core::{
  id::ResourceId,
  resource::{
    DeliverMode, DeliverStep, Deliverable, FlowError, MergeMethod, Pipeline, PipelineEdit, RepositoryAccess, Step,
    StepKey, StepKind, flow_units,
  },
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::{data::Assignables, deliver::integrating_step};

/// What a step runs, as its card's second line says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Runs {
  /// An agent profile with the model the step runs it on: its own, or the profile's.
  Profile {
    name: Arc<str>,
    model: Arc<str>,
  },
  Tool {
    name: Arc<str>,
  },
  NoProfile,
  NoTool,
  /// The step names a profile that has no published version (or none any more).
  ProfileNotPublished {
    id: ResourceId,
  },
  ToolNotPublished {
    id: ResourceId,
  },
  /// A published tool a step cannot run in this version.
  ToolNotRunnable {
    id: ResourceId,
    name: Arc<str>,
  },
}

impl Runs {
  /// Whether the step cannot run as it is, which its card shows in amber.
  pub const fn needs_you(&self) -> bool {
    !matches!(self, Self::Profile { .. } | Self::Tool { .. })
  }

  /// Whether nothing is chosen yet, so the card offers to choose.
  pub const fn is_unassigned(&self) -> bool {
    matches!(self, Self::NoProfile | Self::NoTool)
  }
}

/// Where a step's failure sends the run: the card it returns to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReturnsTo {
  pub number: usize,
  pub name: Arc<str>,
}

/// One step's card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepCard {
  pub key: StepKey,
  /// Its place in the flow, from 1: stage by stage, a stage's steps in the order of the pipeline's steps.
  pub number: usize,
  pub name: Arc<str>,
  pub runs: Runs,
  /// What an agent step may do to the repository; `None` for a tool step.
  pub access: Option<RepositoryAccess>,
  pub on_failure: Option<ReturnsTo>,
  /// The profile or tool `runs` says is not published is gone here (archived or removed), not only unpublished: there
  /// is nothing to publish, the step has to run something else.
  pub gone: bool,
}

impl StepCard {
  /// Whether the step runs a tool, not an agent.
  pub const fn is_tool(&self) -> bool {
    self.access.is_none()
  }
}

/// An approval gate: the run waits for the person after `step`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gate {
  /// The gated step's name, which says what is approved.
  pub step: Arc<str>,
  /// The number of what runs after the gate; `None` when the run ends there.
  pub before: Option<usize>,
}

/// Steps that wait for the same steps, drawn side by side, and the gates after them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stage {
  pub cards: Vec<StepCard>,
  pub gates: Vec<Gate>,
}

/// The Deliver step, numbered after the steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliverCard {
  pub number: usize,
  pub mode: DeliverMode,
  pub method: MergeMethod,
  pub conflicts: Conflicts,
}

/// Who resolves a merge conflict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Conflicts {
  /// A published profile, which the integrating step runs.
  Profile(Arc<str>),
  /// The profile the integrating step runs exists but has no published version, so it cannot resolve anything yet.
  ProfileNotPublished(Arc<str>),
  /// No step can, so conflicts go to the person.
  Person,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineFlow {
  pub stages: Vec<Stage>,
  pub deliver: Option<DeliverCard>,
}

/// How the line ends, which the end marker says: in a merge, or in what the run delivers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
  /// The pipeline has a Deliver step, so the run ends in a merge.
  Merged,
  Delivers(Deliverable),
}

impl Ending {
  pub fn of(pipeline: &Pipeline) -> Self {
    if pipeline.deliver().is_some() {
      Self::Merged
    } else {
      Self::Delivers(pipeline.delivers())
    }
  }
}

impl PipelineFlow {
  /// The flow of `pipeline`, or why its steps cannot be drawn as one line.
  pub fn of(pipeline: &Pipeline, assignables: &Assignables) -> Result<Self, FlowError> {
    let units = flow_units(pipeline.steps())?;
    let steps: HashMap<&StepKey, &Step> = pipeline.steps().iter().map(|step| (&step.key, step)).collect();
    let numbers: HashMap<&StepKey, usize> = units
      .iter()
      .flatten()
      .enumerate()
      .map(|(index, key)| (key, index + 1))
      .collect();
    let total = numbers.len();
    let after = |stage: usize| {
      units
        .get(stage + 1)
        .and_then(|next| next.first())
        .map(|key| numbers[key])
    };
    let mut stages = Vec::with_capacity(units.len());
    for (index, unit) in units.iter().enumerate() {
      let next = after(index).or_else(|| pipeline.deliver().map(|_| total + 1));
      let mut cards = Vec::with_capacity(unit.len());
      let mut gates = Vec::new();
      for key in unit {
        let step = steps[key];
        cards.push(card(step, numbers[key], (&steps, &numbers), assignables));
        if step.approval_gate {
          gates.push(Gate {
            step: step.name.as_str().into(),
            before: next,
          });
        }
      }
      stages.push(Stage { cards, gates });
    }
    Ok(Self {
      stages,
      deliver: pipeline
        .deliver()
        .map(|deliver| deliver_card(pipeline, deliver, total + 1, assignables)),
    })
  }

  /// How many steps the flow has: the count every place that says *Steps N* gives. The Deliver card is numbered after
  /// them but is not a step.
  pub fn len(&self) -> usize {
    self.stages.iter().map(|stage| stage.cards.len()).sum()
  }

  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }
}

fn card(
  step: &Step,
  number: usize,
  (steps, numbers): (&HashMap<&StepKey, &Step>, &HashMap<&StepKey, usize>),
  assignables: &Assignables,
) -> StepCard {
  let access = match &step.kind {
    StepKind::Agent { repository, .. } => Some(*repository),
    StepKind::Tool { .. } => None,
  };
  StepCard {
    key: step.key.clone(),
    number,
    name: step.name.as_str().into(),
    gone: step.kind.executor().is_some_and(|id| assignables.gone(id)),
    runs: runs(step, assignables),
    access,
    on_failure: step.on_failure.as_ref().and_then(|route| {
      Some(ReturnsTo {
        number: *numbers.get(&route.return_to)?,
        name: steps.get(&route.return_to)?.name.as_str().into(),
      })
    }),
  }
}

fn runs(step: &Step, assignables: &Assignables) -> Runs {
  match &step.kind {
    StepKind::Agent { profile: None, .. } => Runs::NoProfile,
    StepKind::Tool { tool: None } => Runs::NoTool,
    StepKind::Agent {
      profile: Some(id),
      model,
      ..
    } => assignables
      .profile(*id)
      .map_or(Runs::ProfileNotPublished { id: *id }, |profile| Runs::Profile {
        name: profile.name.clone(),
        model: model.as_ref().unwrap_or(&profile.model).as_str().into(),
      }),
    StepKind::Tool { tool: Some(id) } => match assignables.tool(*id) {
      None => Runs::ToolNotPublished { id: *id },
      Some(tool) if !tool.runnable => Runs::ToolNotRunnable {
        id: *id,
        name: tool.name.clone(),
      },
      Some(tool) => Runs::Tool {
        name: tool.name.clone(),
      },
    },
  }
}

fn deliver_card(pipeline: &Pipeline, deliver: &DeliverStep, number: usize, assignables: &Assignables) -> DeliverCard {
  let conflicts = integrating_step(pipeline, deliver, |_| true)
    .and_then(|step| step.kind.executor())
    .map_or(Conflicts::Person, |id| match assignables.profile(id) {
      Some(profile) => Conflicts::Profile(profile.name.clone()),
      None => assignables
        .draft_profiles
        .iter()
        .find(|draft| draft.id == id)
        .map_or(Conflicts::Person, |draft| {
          Conflicts::ProfileNotPublished(draft.name.clone())
        }),
    });
  DeliverCard {
    number,
    mode: deliver.mode(),
    method: deliver.method(),
    conflicts,
  }
}

/// A pipeline's flow as the page reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowRead {
  /// Whether the steps are the draft's; otherwise the latest published version's.
  pub draft: bool,
  /// How the line ends, which is known even when the steps cannot be drawn.
  pub ending: Ending,
  /// The flow, or why the steps cannot be drawn as one line.
  pub flow: Result<PipelineFlow, FlowError>,
}

/// A pipeline as it is stored, read for the page: what it draws, and what an edit of the page starts from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stored {
  pub read: FlowRead,
  /// The steps the flow was read from: the draft's, or the latest version's.
  pub body: Pipeline,
  /// Whether `body` differs from the latest published version, or none was published: a save has something to
  /// publish. A stored draft that equals the latest version has nothing.
  pub unpublished: bool,
}

impl FlowRead {
  /// Pipeline `id` as stored: its draft when it has one, otherwise its latest version; `None` when it has neither.
  pub fn of(db: &ProjectDb, id: ResourceId, assignables: &Assignables) -> Result<Option<Stored>, StoreError> {
    let latest = db.latest_version::<Pipeline>(id)?;
    let (body, draft) = match db.draft::<Pipeline>(id)? {
      Some(draft) => (draft.body().clone(), true),
      None => match &latest {
        Some(version) => (version.body().clone(), false),
        None => return Ok(None),
      },
    };
    let unpublished = latest.as_ref().is_none_or(|version| version.body() != &body);
    Ok(Some(Stored {
      read: Self {
        draft,
        ending: Ending::of(&body),
        flow: PipelineFlow::of(&body, assignables),
      },
      body,
      unpublished,
    }))
  }

  /// The flow of `edit`, which the page draws while it holds the edit. Steps that break a rule leave no flow
  /// ([`FlowError::Broken`]); how the line ends is known all the same.
  pub fn of_edit(edit: &PipelineEdit, assignables: &Assignables) -> Self {
    let (ending, flow) = match edit.validate() {
      Ok(body) => (Ending::of(&body), PipelineFlow::of(&body, assignables)),
      Err(error) => (
        edit
          .deliver()
          .map_or(Ending::Delivers(edit.delivers()), |_| Ending::Merged),
        Err(FlowError::Broken(error)),
      ),
    };
    Self {
      draft: true,
      ending,
      flow,
    }
  }
}
