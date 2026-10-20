//! The notices that the run's base branch moved (ORC-54, `orchester_core::base_move`) as the timeline shows them: an
//! Orchester event, *master moved to 1a2b3c4 · 2 tasks landed · merge requested*, with where the notice stands. One
//! that waits sits with the messages that wait for a step; one delivered sits in the step whose agent read it.

use std::sync::Arc;

use orchester_core::{base_move::BaseNotice, id::StepExecutionId, run::RunStatus};

use super::{Item, ItemKind};
use crate::tasks::detail::RunView;

/// A notice and where it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseMovedMark {
  pub notice: BaseNotice,
  pub state: NoticeState,
}

/// Where a notice stands, as its event says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoticeState {
  /// It waits for the run's next agent of a writing step: the one at work after its turn, or the next to start.
  WritingStep,
  /// It is kept until the run goes on: it is paused, stopped at a limit, waits for a person or was interrupted.
  RunGoesOn,
  /// The run is ready for review: a follow-up that asks an agent of a writing step reads it.
  FollowUp,
  /// The agent `agent` of step `step` read it.
  Delivered { agent: Arc<str>, step: Arc<str> },
}

/// The run's notices that belong to `execution`: those delivered to it, or with `None` those still waiting.
pub(super) fn items(run: &RunView, execution: Option<StepExecutionId>) -> impl Iterator<Item = Item> + '_ {
  all(run).filter(move |item| item.execution == execution)
}

/// Every notice of the run, oldest first, each where it stands: in the step whose agent read it, or waiting.
pub fn all(run: &RunView) -> impl Iterator<Item = Item> + '_ {
  run.base_notices.iter().map(|notice| {
    let (state, at) = match notice.delivered() {
      Some((execution, at)) => {
        let step = run.step_of(execution);
        let state = NoticeState::Delivered {
          agent: step.and_then(|step| step.agent.clone()).unwrap_or_default(),
          step: step.map(|step| step.name.clone()).unwrap_or_default(),
        };
        (state, at)
      }
      None => (waiting(run), notice.noticed_at),
    };
    Item {
      key: format!("base-moved-{}", notice.id).into(),
      execution: notice.delivered().map(|(to, _)| to),
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
    RunStatus::Pending | RunStatus::Running | RunStatus::Failed(_) | RunStatus::Cancelled => NoticeState::WritingStep,
  }
}
