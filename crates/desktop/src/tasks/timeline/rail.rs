//! The step rail of the Conversation tab (`docs/design/orchester-workspace.md`, "Decided: C with the
//! rail on the right", `kpnmP`): one row per step of the run's pipeline, in its order, after *All
//! steps*. A row tells its latest execution that ran: what it waits for, whether it runs, passed,
//! failed or returned the work, or that the step has not started. Picking a row shows the step's
//! executions, every attempt's.

use std::sync::Arc;

use orchester_core::{
  id::StepExecutionId,
  resource::StepKey,
  run::{RunStatus, StepStatus, WaitReason},
};

use super::{Item, ItemKind, StepHead, mark::MarkKind};
use crate::tasks::detail::RunView;

/// What a step's row says of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RailState {
  NotStarted,
  Working,
  /// Its plan, a tool request or a question waits for the person.
  Waits(WaitReason),
  Passed {
    /// Its plan was approved by the person.
    approved: bool,
  },
  /// A tool step's failing command's exit code, when it had one.
  Failed {
    exit: Option<i64>,
  },
  /// It failed and a failure route returned the work to the step named.
  Returned {
    to: Arc<str>,
  },
  /// Its attempt ended before it finished: a retry, a failure beside it or a cancel.
  Stopped,
}

/// One step of the rail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailStep {
  pub key: StepKey,
  pub number: usize,
  pub name: Arc<str>,
  /// The agent role; `None` for a tool step, or once its profile is gone.
  pub agent: Option<Arc<str>>,
  pub tool_step: bool,
  /// Its executions that ran, in the order they ran.
  pub executions: Vec<StepExecutionId>,
  pub state: RailState,
  /// The latest execution's header, once one ran.
  pub latest: Option<StepHead>,
}

impl RailStep {
  pub fn started(&self) -> bool {
    !self.executions.is_empty()
  }
}

/// What *All steps* says: the executions and tokens of the run, or that it is queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailSummary {
  Queued {
    steps: usize,
  },
  /// Some steps have not run (yet).
  Partial {
    ran: usize,
    steps: usize,
    tokens: u64,
  },
  Ran {
    executions: usize,
    tokens: u64,
  },
}

/// The rail of `run`, from its timeline `items`.
pub fn rail(run: &RunView, items: &[Item]) -> Vec<RailStep> {
  let heads: Vec<(usize, &StepHead)> = items
    .iter()
    .enumerate()
    .filter_map(|(index, item)| match &item.kind {
      ItemKind::Step(head) if head.execution.is_some() => Some((index, head)),
      _ => None,
    })
    .collect();
  run
    .steps
    .iter()
    .map(|step| {
      let own: Vec<&(usize, &StepHead)> = heads.iter().filter(|(_, head)| head.number == step.number).collect();
      let latest = own.last().copied();
      RailStep {
        key: step.key.clone(),
        number: step.number,
        name: step.name.clone(),
        agent: step.agent.clone().filter(|_| !step.tool_step),
        tool_step: step.tool_step,
        executions: own.iter().filter_map(|(_, head)| head.execution).collect(),
        state: latest.map_or(RailState::NotStarted, |(index, head)| state(items, *index, head)),
        latest: latest.map(|(_, head)| (*head).clone()),
      }
    })
    .collect()
}

/// The state of the step whose latest header is `items[index]`.
fn state(items: &[Item], index: usize, head: &StepHead) -> RailState {
  match head.status {
    Some(StepStatus::Waiting(reason)) => RailState::Waits(reason),
    Some(StepStatus::Passed) => RailState::Passed {
      approved: head.approved,
    },
    Some(StepStatus::Failed) => returned_to(items, index, head).map_or_else(
      || RailState::Failed {
        exit: head.tool_run.as_ref().and_then(|run| run.exit_code),
      },
      |to| RailState::Returned { to },
    ),
    Some(StepStatus::Skipped) => RailState::Stopped,
    Some(StepStatus::Running | StepStatus::Pending) | None => RailState::Working,
  }
}

/// The step a later attempt's return started from, when the step of `head` failed and returned it.
fn returned_to(items: &[Item], index: usize, head: &StepHead) -> Option<Arc<str>> {
  items[index + 1..].iter().find_map(|item| match &item.kind {
    ItemKind::Mark(mark) => match &mark.kind {
      MarkKind::Returned { to, failed, .. } if *failed == head.step => Some(to.clone()),
      _ => None,
    },
    _ => None,
  })
}

/// What *All steps* says of `steps` of `run`.
pub fn summary(run: &RunView, steps: &[RailStep]) -> RailSummary {
  let ran = steps.iter().filter(|step| step.started()).count();
  if ran == 0 && *run.run.status() == RunStatus::Pending {
    return RailSummary::Queued { steps: steps.len() };
  }
  let tokens = run.usage().counted();
  if ran < steps.len() {
    return RailSummary::Partial {
      ran,
      steps: steps.len(),
      tokens,
    };
  }
  RailSummary::Ran {
    executions: steps.iter().map(|step| step.executions.len()).sum(),
    tokens,
  }
}
