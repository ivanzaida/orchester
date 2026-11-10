//! What keeps a pipeline from running as it is, in the order the page's *Ready?* lists it (`Pipelines · v2` P1
//! `j0OZhl`, P8 `eA5ed`): the steps whose card stands in amber, then what the readiness read says this computer lacks
//! ([`crate::readiness`]), then why it cannot be told. The readiness is the hub's own; nothing is computed again here
//! except which steps of the flow cannot run, which the flow already knows ([`Runs::needs_you`]).

use std::sync::Arc;

use orchester_core::{resource::StepKey, sharing::Missing};

use super::flow::{FlowRead, PipelineFlow, Runs};
use crate::readiness::{ReadinessStanding, Undecided};

/// A step that cannot run as it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepProblem {
  pub key: StepKey,
  /// Its number on the flow, as its card carries it.
  pub number: usize,
  pub name: Arc<str>,
  pub runs: Runs,
}

/// One thing a pipeline lacks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
  /// A step has no profile or tool, or names one that cannot run.
  Step(StepProblem),
  /// A part of a profile or tool its steps run that this computer lacks.
  Missing(Missing),
  /// Whether it can run cannot be known, and why.
  Undecided(Undecided),
}

/// Everything that keeps the pipeline whose steps are `read` from running as it is, given how the readiness read
/// ([`crate::readiness`]) says its latest version stands; empty when it is ready. A draft stands for the steps while
/// the pipeline has one and the readiness is about its latest version, so a published pipeline with a draft can list
/// both. When a step already says what is wrong, the readiness's own *cannot tell* is left out: it is the same step,
/// named another way.
pub fn problems(read: &FlowRead, standing: Option<&ReadinessStanding>) -> Vec<Problem> {
  let mut problems: Vec<Problem> = step_problems(read).into_iter().map(Problem::Step).collect();
  match standing {
    Some(ReadinessStanding::Missing(readiness)) => {
      problems.extend(readiness.missing().iter().cloned().map(Problem::Missing));
    }
    Some(ReadinessStanding::Undecided(why)) if problems.is_empty() => {
      problems.push(Problem::Undecided(why.clone()));
    }
    Some(ReadinessStanding::Ready | ReadinessStanding::Undecided(_)) | None => {}
  }
  problems
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
