//! What keeps a pipeline from running as it is, in the order the page's *Ready?* lists it (`Pipelines · v2` P1
//! `j0OZhl`, P8 `eA5ed`): what is wrong with the draft the page shows, the steps whose card stands in amber, then what
//! the readiness read says this computer lacks ([`crate::readiness`]), then every profile or tool it cannot tell. The
//! readiness is the hub's own; nothing is computed again here except which steps of the flow cannot run, which the flow
//! already knows ([`Blocked`]).
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

use super::flow::{FlowRead, Runs, StepCard};
use crate::readiness::{ProjectReadiness, ReadinessStanding, Undecided};

/// A step that cannot run as it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepProblem {
  pub key: StepKey,
  /// Its number on the flow, as its card carries it; in the order of the steps when the flow cannot draw them.
  pub number: usize,
  pub name: Arc<str>,
  pub blocked: Blocked,
}

/// What keeps one step from running, which its card shows in amber. A step that can run has none, so nothing that
/// says what a step lacks can be asked about one that lacks nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blocked {
  NoProfile,
  NoTool,
  /// The step names a profile that has no published version yet.
  ProfileNotPublished {
    id: ResourceId,
  },
  /// The step names a profile or tool that is gone here (archived or removed): it has to run another.
  Gone {
    id: ResourceId,
    tool: bool,
  },
  /// The step's profile runs on a connection the project no longer has.
  ProfileConnectionGone {
    id: ResourceId,
    name: Arc<str>,
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

impl Blocked {
  /// What keeps the step of `card` from running; `None` when it can run.
  pub fn of_card(card: &StepCard) -> Option<Self> {
    match card.runs {
      Runs::ProfileNotPublished { id } if card.gone => Some(Self::Gone { id, tool: false }),
      Runs::ToolNotPublished { id } if card.gone => Some(Self::Gone { id, tool: true }),
      _ => Self::of(&card.runs),
    }
  }

  /// What keeps a step that runs `runs` from running; `None` when it can run.
  pub fn of(runs: &Runs) -> Option<Self> {
    Some(match runs {
      Runs::Profile { .. } | Runs::Tool { .. } => return None,
      Runs::NoProfile => Self::NoProfile,
      Runs::NoTool => Self::NoTool,
      Runs::ProfileNotPublished { id } => Self::ProfileNotPublished { id: *id },
      Runs::ProfileConnectionGone { id, name } => Self::ProfileConnectionGone {
        id: *id,
        name: name.clone(),
      },
      Runs::ToolNotPublished { id } => Self::ToolNotPublished { id: *id },
      Runs::ToolNotRunnable { id, name } => Self::ToolNotRunnable {
        id: *id,
        name: name.clone(),
      },
    })
  }

  /// Whether nothing is chosen yet, so the step is opened to choose.
  pub const fn is_unassigned(&self) -> bool {
    matches!(self, Self::NoProfile | Self::NoTool)
  }

  /// The profile or tool the step names, when it names one.
  pub const fn names(&self) -> Option<ResourceId> {
    match self {
      Self::NoProfile | Self::NoTool => None,
      Self::ProfileNotPublished { id }
      | Self::Gone { id, .. }
      | Self::ProfileConnectionGone { id, .. }
      | Self::ToolNotPublished { id }
      | Self::ToolNotRunnable { id, .. } => Some(*id),
    }
  }
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
  /// The profiles and tools the steps the page shows run, while that is a draft over a published version: a profile
  /// or tool of the version that the draft no longer runs is not named. `None` lists every one.
  pub runs: Option<&'a [ResourceId]>,
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
    Some(ReadinessStanding::Undecided(why)) => found.extend(undecided(why, (basis.undecided, &steps), basis.runs)),
    Some(ReadinessStanding::Ready) | None => {}
  }
  if found.is_empty() && basis.flow.draft && !basis.published {
    found.push(Problem::Draft(Draft::NotPublished));
  }
  found
}

/// The profiles and tools named when the pipeline is undecided: each undecided one, else the reason as it stands. A
/// profile or tool a step names already says what keeps it (no version here, archived, no connection), so it is not
/// named twice; nor is one that `runs` (the draft's) leaves out.
fn undecided(
  why: &Undecided,
  (members, steps): (&[UndecidedMember], &[StepProblem]),
  runs: Option<&[ResourceId]>,
) -> Vec<Problem> {
  let named = |id: ResourceId| steps.iter().any(|step| step.blocked.names() == Some(id));
  let left_out = |id: ResourceId| runs.is_some_and(|runs| !runs.contains(&id));
  if matches!(why, Undecided::RunsUnknown(id) | Undecided::RunsArchived(id, _) if named(*id))
    || matches!(why, Undecided::RunsUnknown(id) | Undecided::RunsArchived(id, _) if left_out(*id))
  {
    return Vec::new();
  }
  if members.is_empty() || !matches!(why, Undecided::Member { .. }) {
    return vec![Problem::Undecided(why.clone())];
  }
  members
    .iter()
    .filter(|member| !named(member.id) && !left_out(member.id))
    .cloned()
    .map(Problem::Member)
    .collect()
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

/// The steps of `read` that cannot run as they are, in the order of the flow (of the steps, when it cannot draw them).
pub fn step_problems(read: &FlowRead) -> Vec<StepProblem> {
  let cards = match &read.flow {
    Ok(flow) => flow.stages.iter().flat_map(|stage| &stage.cards).collect::<Vec<_>>(),
    Err(_) => read.unplaced.iter().collect(),
  };
  cards.into_iter().filter_map(of_card).collect()
}

fn of_card(card: &StepCard) -> Option<StepProblem> {
  Some(StepProblem {
    key: card.key.clone(),
    number: card.number,
    name: card.name.clone(),
    blocked: Blocked::of_card(card)?,
  })
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
