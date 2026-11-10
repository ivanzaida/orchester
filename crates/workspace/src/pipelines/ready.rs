//! What keeps a pipeline from running as it is, in the order the page's *Ready?* lists it (`Pipelines · v2` P1
//! `j0OZhl`, P8 `eA5ed`): what is wrong with the draft the page shows, the steps whose card stands in amber, then what
//! the readiness read says this computer lacks ([`crate::readiness`]), then every profile or tool it cannot tell. The
//! readiness is the hub's own; nothing is computed again here except which steps of the flow cannot run, which the flow
//! already knows ([`Runs::needs_you`]).
//!
//! The page shows a draft when the pipeline has one, so what it lists about the steps is the draft's, while the
//! readiness is about the latest published version (the one tasks use).

use std::sync::Arc;

use orchester_core::{
  id::ResourceId,
  resource::{FlowError, Pipeline, ResourceKind, StepKey},
  sharing::Missing,
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};

use super::flow::{FlowRead, PipelineFlow, Runs};
use crate::readiness::{ProjectReadiness, ReadinessStanding, Undecided};

/// A step that cannot run as it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepProblem {
  pub key: StepKey,
  /// Its number on the flow, as its card carries it.
  pub number: usize,
  pub name: Arc<str>,
  pub runs: Runs,
}

/// A profile or tool the latest version's steps run (or its integrator) that cannot be told to be ready.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndecidedMember {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub kind: ResourceKind,
  pub why: Undecided,
}

/// What is wrong with the draft as a whole, not with one of its steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Draft {
  /// It has no steps: the person adds one.
  NoSteps,
  /// Its steps wait for each other, or break another rule, so they have no flow.
  Broken,
  /// A step waits for only some of what runs before it, which one line cannot draw.
  Branches,
  /// Nothing is wrong with it, but it was never published, so tasks cannot use it.
  NotPublished,
}

/// One thing a pipeline lacks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
  Draft(Draft),
  /// A step has no profile or tool, or names one that cannot run.
  Step(StepProblem),
  /// A part of a profile or tool its steps run that this computer lacks.
  Missing(Missing),
  /// A profile or tool its steps run that cannot be told to be ready, and why.
  Member(UndecidedMember),
  /// Whether it can run cannot be known, and why, without naming a profile or tool its steps run.
  Undecided(Undecided),
}

/// What [`problems`] reads of a pipeline.
#[derive(Clone, Copy, Debug)]
pub struct Basis<'a> {
  /// Its steps as the page draws them: the draft's when it has one.
  pub flow: &'a FlowRead,
  /// How its latest version stands; `None` while it has none.
  pub standing: Option<&'a ReadinessStanding>,
  /// Every profile or tool of the latest version that cannot be told ([`undecided_members`]).
  pub undecided: &'a [UndecidedMember],
  /// Whether it has a published version.
  pub published: bool,
}

/// Everything that keeps the pipeline from running as it is; empty when it is ready.
pub fn problems(basis: Basis<'_>) -> Vec<Problem> {
  let steps = step_problems(basis.flow);
  let mut found: Vec<Problem> = draft_problem(basis).map(Problem::Draft).into_iter().collect();
  found.extend(steps.iter().cloned().map(Problem::Step));
  match basis.standing {
    Some(ReadinessStanding::Missing(readiness)) => {
      found.extend(readiness.missing().iter().cloned().map(Problem::Missing));
    }
    Some(ReadinessStanding::Undecided(why)) => found.extend(undecided(why, (basis.undecided, &steps))),
    Some(ReadinessStanding::Ready) | None => {}
  }
  if found.is_empty() && basis.flow.draft && !basis.published {
    found.push(Problem::Draft(Draft::NotPublished));
  }
  found
}

/// The profiles and tools named when the pipeline is undecided: each undecided one, else the reason as it stands. A
/// profile or tool a step names that has no version here is that step's own problem already, so the *cannot tell* about
/// it is left out.
fn undecided(why: &Undecided, (members, steps): (&[UndecidedMember], &[StepProblem])) -> Vec<Problem> {
  if matches!(why, Undecided::RunsUnknown(id) if steps.iter().any(|step| names(&step.runs, *id))) {
    return Vec::new();
  }
  if members.is_empty() || !matches!(why, Undecided::Member { .. }) {
    return vec![Problem::Undecided(why.clone())];
  }
  members.iter().cloned().map(Problem::Member).collect()
}

/// Whether a card says it names the profile or tool `id`, which has no published version.
fn names(runs: &Runs, id: ResourceId) -> bool {
  matches!(runs, Runs::ProfileNotPublished { id: named } | Runs::ToolNotPublished { id: named } if *named == id)
}

/// What is wrong with the draft the page shows, besides its steps; none when the page shows a published version.
fn draft_problem(basis: Basis<'_>) -> Option<Draft> {
  if !basis.flow.draft {
    return None;
  }
  match &basis.flow.flow {
    Ok(flow) if flow.is_empty() => Some(Draft::NoSteps),
    Ok(_) => None,
    Err(FlowError::Branches(_)) => Some(Draft::Branches),
    Err(FlowError::Broken(_)) => Some(Draft::Broken),
  }
}

/// The steps of `read` that cannot run as they are, in the order of the flow.
pub fn step_problems(read: &FlowRead) -> Vec<StepProblem> {
  read.flow.as_ref().map_or_else(|_| Vec::new(), of_flow)
}

fn of_flow(flow: &PipelineFlow) -> Vec<StepProblem> {
  flow
    .stages
    .iter()
    .flat_map(|stage| &stage.cards)
    .filter(|card| card.runs.needs_you())
    .map(|card| StepProblem {
      key: card.key.clone(),
      number: card.number,
      name: card.name.clone(),
      runs: card.runs.clone(),
    })
    .collect()
}

/// Every profile or tool the latest version of `pipeline` runs (and its integrator) that `readiness` cannot tell, in
/// the order of the steps; the readiness itself names only the first.
pub fn undecided_members(
  db: &ProjectDb,
  pipeline: ResourceId,
  readiness: &ProjectReadiness,
) -> Result<Vec<UndecidedMember>, StoreError> {
  let Some(version) = db.latest_version::<Pipeline>(pipeline)? else {
    return Ok(Vec::new());
  };
  let body = version.body();
  let integrator = body.deliver().and_then(|deliver| deliver.integrator());
  let mut members: Vec<UndecidedMember> = Vec::new();
  for id in body
    .steps()
    .iter()
    .filter_map(|step| step.kind.executor())
    .chain(integrator)
  {
    if members.iter().any(|member| member.id == id) {
      continue;
    }
    let Some(found) = readiness.resource(id) else {
      continue;
    };
    if let ReadinessStanding::Undecided(why) = found.standing() {
      members.push(UndecidedMember {
        id,
        name: found.name().into(),
        kind: found.kind(),
        why: why.clone(),
      });
    }
  }
  Ok(members)
}
