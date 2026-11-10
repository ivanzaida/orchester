//! A pipeline as the Pipelines page draws its steps (`Pipelines · v2` P1 `s7LMyQ`, P8 `vujkk`): the steps in the
//! order they run in, the steps that run side by side in one stage, the approval gates on the line after their step,
//! the failure routes by the numbers the cards carry, and the Deliver step numbered last. The layout is derived from
//! the steps' `depends_on` ([`orchester_core::resource::flow_units`]); nothing of it is stored.

use std::{collections::HashMap, path::Path, sync::Arc};

use orchester_core::{
  id::ResourceId,
  resource::{
    DeliverMode, DeliverStep, Deliverable, FlowError, MergeMethod, Pipeline, RepositoryAccess, Step, StepKey, StepKind,
    flow_units,
  },
  store::ResourceStore,
};
use orchester_store::ProjectDb;

use super::{
  data::{Assignables, OpenError},
  deliver::integrating_step,
  write::WriteError,
};
use crate::messages::Message;

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
  ProfileNotPublished,
  ToolNotPublished,
  /// A published tool a step cannot run in this version.
  ToolNotRunnable {
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
  /// The profile that resolves conflicts; `None` when no step can, so conflicts go to the person.
  pub conflicts: Option<Arc<str>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineFlow {
  pub stages: Vec<Stage>,
  pub deliver: Option<DeliverCard>,
  /// What a run of the pipeline delivers, which the line's end says.
  pub delivers: Deliverable,
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
      delivers: pipeline.delivers(),
    })
  }

  /// How many numbered cards the flow has, the Deliver step among them.
  pub fn len(&self) -> usize {
    self.stages.iter().map(|stage| stage.cards.len()).sum::<usize>() + usize::from(self.deliver.is_some())
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
      .map_or(Runs::ProfileNotPublished, |profile| Runs::Profile {
        name: profile.name.clone(),
        model: model.as_ref().unwrap_or(&profile.model).as_str().into(),
      }),
    StepKind::Tool { tool: Some(id) } => match assignables.tool(*id) {
      None => Runs::ToolNotPublished,
      Some(tool) if !tool.runnable => Runs::ToolNotRunnable {
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
    .and_then(|profile| assignables.profile(profile))
    .map(|profile| profile.name.clone());
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
  /// The flow, or why the steps cannot be drawn as one line.
  pub flow: Result<PipelineFlow, FlowError>,
}

impl FlowRead {
  /// The flow of pipeline `id` of the project at `repo_root`: its draft when it has one, otherwise its latest
  /// version. Blocks on SQLite: run it on the background pool.
  pub fn load_in(repo_root: &Path, id: ResourceId) -> Result<Self, Message> {
    let db = ProjectDb::open(repo_root)
      .map_err(OpenError::from)
      .map_err(work_error)?;
    Self::load(&db, id).map_err(work_error)
  }

  fn load(db: &ProjectDb, id: ResourceId) -> Result<Self, OpenError> {
    let draft = db.draft::<Pipeline>(id)?;
    let (body, drafted) = match draft {
      Some(draft) => (draft.body().clone(), true),
      None => {
        let version = db.latest_version::<Pipeline>(id)?.ok_or(OpenError::Missing)?;
        (version.body().clone(), false)
      }
    };
    let assignables = Assignables::load(db)?;
    Ok(Self {
      draft: drafted,
      flow: PipelineFlow::of(&body, &assignables),
    })
  }
}

fn work_error(error: OpenError) -> Message {
  WriteError::from(error).message()
}
