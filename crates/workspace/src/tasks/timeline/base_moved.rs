//! The notices that the run's base branch moved (ORC-54, `orchester_core::base_move`) as the timeline shows them: an
//! Orchester event, *master moved to 1a2b3c4 · 2 tasks landed · merge requested*, with where the notice stands. One
//! that waits sits with the messages that wait for a step; one delivered sits in the step whose agent read it, and one
//! the runner merged itself, as it overlapped none of the run's changes (ORC-151), in the step that started on the
//! merged base. Only a step that implements the change takes it (ORC-65,
//! `orchester_core::resource::Pipeline::takes_base_moves`): a run past those steps, with only Review or Verify ahead,
//! keeps it until its work goes back to one of them, unless the runner merges it first.

use std::sync::Arc;

use orchester_core::{base_move::BaseNotice, id::StepExecutionId, run::RunStatus};

use super::{Item, ItemKind};
use crate::tasks::detail::{RunView, StepView};

/// A notice and where it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseMovedMark {
  pub notice: BaseNotice,
  pub state: NoticeState,
}

/// Where a notice stands, as its event says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoticeState {
  /// It waits for the run's next agent of a step that merges the base: the one at work after its turn, or the next to
  /// start.
  WritingStep,
  /// The run's steps that merge the base are done in this attempt and only steps that do not are ahead: it waits until
  /// the work goes back to `step` (a return, a failure route, or *Ask an agent to resolve*).
  WorkGoesBack { step: Arc<str> },
  /// It is kept until the run goes on: it is paused, stopped at a limit, waits for a person or was interrupted.
  RunGoesOn,
  /// The run is ready for review: a follow-up that asks an agent of a step that merges the base reads it.
  FollowUp,
  /// The agent `agent` of step `step` read it.
  Delivered { agent: Arc<str>, step: Arc<str> },
  /// The runner merged it itself before step `step` started, as it overlapped none of the run's changes.
  MergedByRunner { step: Arc<str> },
}

/// The run's notices that belong to `execution`: those delivered to it or merged before it, or with `None` those
/// still waiting.
pub(super) fn items(run: &RunView, execution: Option<StepExecutionId>) -> impl Iterator<Item = Item> + '_ {
  all(run).filter(move |item| item.execution == execution)
}

/// Every notice of the run, oldest first, each where it stands: in the step whose agent read it or that started on
/// the base the runner merged, or waiting.
pub fn all(run: &RunView) -> impl Iterator<Item = Item> + '_ {
  run.base_notices.iter().map(|notice| {
    let (state, at) = match notice.settled() {
      Some((execution, at)) => {
        let step = run.step_of(execution);
        let name = step.map(|step| step.name.clone()).unwrap_or_default();
        let state = if notice.merged().is_some() {
          NoticeState::MergedByRunner { step: name }
        } else {
          NoticeState::Delivered {
            agent: step.and_then(|step| step.agent.clone()).unwrap_or_default(),
            step: name,
          }
        };
        (state, at)
      }
      None => (waiting(run), notice.noticed_at),
    };
    Item {
      key: format!("base-moved-{}", notice.id).into(),
      execution: notice.settled().map(|(to, _)| to),
      at,
      kind: ItemKind::BaseMoved(BaseMovedMark {
        notice: notice.clone(),
        state,
      }),
    }
  })
}

/// What a pending notice of `run` waits for.
fn waiting(run: &RunView) -> NoticeState {
  match run.run.status() {
    RunStatus::Succeeded => NoticeState::FollowUp,
    RunStatus::Paused(_) | RunStatus::WaitingForPerson(_) | RunStatus::Interrupted(_) => NoticeState::RunGoesOn,
    RunStatus::Running => past_merging_steps(run).map_or(NoticeState::WritingStep, |step| NoticeState::WorkGoesBack {
      step: step.name.clone(),
    }),
    RunStatus::Pending | RunStatus::Failed(_) | RunStatus::Cancelled => NoticeState::WritingStep,
  }
}

/// The first of `run`'s steps that merge the base when it moves, when the run's current attempt has none of them
/// ahead or at work: each ran, and only steps that never take the notice, such as Review and Verify, are left.
/// `None` while one is ahead, or when the pipeline has none.
fn past_merging_steps(run: &RunView) -> Option<&StepView> {
  let pipeline = run.run.snapshot().body();
  let mut merging = run
    .steps
    .iter()
    .filter(|step| step.writes() && pipeline.takes_base_moves(&step.key))
    .peekable();
  let first = *merging.peek()?;
  let attempt = run.run.current_attempt()?;
  let ahead = merging.any(|step| {
    attempt
      .executions()
      .iter()
      .any(|execution| execution.step() == &step.key && execution.status().is_open())
  });
  (!ahead).then_some(first)
}
