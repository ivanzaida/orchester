//! What a card dropped on a column does (`What it adds` `bNMv9`, the person's rules of 2026-10-26): by status, the
//! workspace's rules ([`moves::decide`]): a drop that changes nothing stays, a refused one says why at the column's
//! top, Done and Canceled close the task through the screen's closer (its run stopped first after the confirmation in
//! place), Done asks first for work that waits to be merged, Todo asks first when queuing starts the task at once and
//! asks a task whose run ended whether it runs again, only moves to Todo or stays. By priority or module a drop sets
//! them; by pipeline it is refused, since a pipeline is chosen on the task's page. A drop into another row of the board
//! sets the row's module or priority too. The Move to menu and the keys move a card the same way.

use std::sync::Arc;

use orchester_core::task::organization::WorkflowStatus;
use orchester_workspace::tasks::{
  close::Closing,
  moves::{self, Decision, Move},
};

use super::{
  actions::{Ask, BoardState, Change, ChangeKind, Refusal, Refused, Target},
  card::CardModel,
  display::Lanes,
  model::ColumnKey,
};
use crate::screens::task_close::{Closer, Subject};

/// What a drop is decided with, and what a card's menu offers; the board provides it to its cards for this render.
#[derive(Clone)]
pub struct Dropping {
  pub state: BoardState,
  pub closer: Closer,
  pub lanes: Lanes,
  /// Free agent slots while the runner serves the project and new starts are not held; `None` otherwise.
  pub free: Option<usize>,
  /// The project is shared on a team server.
  pub shared: bool,
  /// The person is no longer a member of the shared project: every card is read-only.
  pub read_only: bool,
  /// The project's tags, by id and name.
  pub tags: Arc<Vec<(orchester_core::id::TagId, Arc<str>)>>,
}

impl Dropping {
  pub fn of(ctx: &mut lurq::app::ctx::Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the board provides how its drops go before mounting its cards")
  }

  /// Whether `card` is read-only here, as it is in the list: the person is no longer a member of the project, or the
  /// task is kept *Not shared*. It does not move, is not selected and its menu changes nothing.
  pub fn locked(&self, card: &CardModel) -> bool {
    self.read_only || card.row.not_shared
  }

  /// Drops `card` on `target`; a read-only card stays.
  pub fn drop_card(&self, card: &CardModel, target: &Target) {
    if self.locked(card) {
      return;
    }
    self.state.refused.set(None);
    let lane = self.lane_change(card, target);
    let changes = match &target.column {
      ColumnKey::Pipeline(_) => return self.refuse(card, target, Refusal::Pipeline),
      ColumnKey::Priority(priority) => {
        (card.row.organization.priority() != *priority).then_some(ChangeKind::Priority(*priority))
      }
      ColumnKey::Module(module) => (card.row.module != *module).then_some(ChangeKind::Module(*module)),
      ColumnKey::Status(status) => match self.status_drop(card, target, *status) {
        Some(kind) => kind,
        None => return,
      },
    };
    let kinds: Vec<ChangeKind> = changes.into_iter().chain(lane).collect();
    self.state.change(changes_of(card, kinds));
  }

  /// What a drop on the status column `status` changes, after what it asks or refuses; `None` when the drop is taken
  /// elsewhere (asked in place, closed through the closer) or refused, `Some(None)` when it changes no status.
  fn status_drop(&self, card: &CardModel, target: &Target, status: WorkflowStatus) -> Option<Option<ChangeKind>> {
    let subject = self.subject(card);
    match moves::decide(&card.row, status) {
      Decision::Stay => Some(None),
      Decision::Refused(refusal) => {
        let refusal = match refusal {
          moves::Refusal::RunNotEnded => Refusal::RunNotEnded,
          moves::Refusal::Incomplete => Refusal::Incomplete(card.readiness.clone()),
          moves::Refusal::HasRun => Refusal::HasRun,
        };
        self.refuse(card, target, refusal);
        None
      }
      Decision::Move(Move::Close(Closing::Done)) if moves::leaves_unmerged(&card.row) => {
        self.state.asked.set(Some(Ask::Unmerged(subject)));
        None
      }
      Decision::Move(Move::Close(closing)) => {
        self.closer.close(subject, closing);
        None
      }
      Decision::Move(Move::Queue) => match self.free.filter(|free| *free > 0) {
        Some(free) => {
          self.state.asked.set(Some(Ask::StartsNow {
            task: subject,
            free,
            pipeline: card.pipeline.clone(),
          }));
          None
        }
        None => Some(Some(ChangeKind::Queue)),
      },
      Decision::Move(Move::RunAgain) => {
        if !self.ask_run_again(card) {
          self.refuse(card, target, Refusal::HasRun);
        }
        None
      }
      Decision::Move(Move::ReturnToDraft) => Some(Some(ChangeKind::ReturnToDraft)),
      Decision::Move(Move::Status(status)) => Some(Some(ChangeKind::Status(status))),
    }
  }

  /// Asks at `card` whether its task, whose latest run ended, runs again (or, unless it is in Todo already, only moves
  /// there); `false` when that run cannot be read. A drop on Todo and the card menu's Run again ask it.
  pub fn ask_run_again(&self, card: &CardModel) -> bool {
    let Some(after) = card.latest_run else {
      return false;
    };
    self.state.asked.set(Some(Ask::RunAgain {
      task: self.subject(card),
      after,
      revision: card.revision,
      pipeline: card.pipeline.clone(),
    }));
    true
  }

  /// The module or priority of another row of the board the drop went into.
  fn lane_change(&self, card: &CardModel, target: &Target) -> Option<ChangeKind> {
    let lane = target.lane.as_deref()?;
    match self.lanes {
      Lanes::None => None,
      Lanes::Module => {
        let module = lane.strip_prefix("module-")?;
        let module = if module == "none" {
          None
        } else {
          Some(module.parse().ok()?)
        };
        (card.row.module != module).then_some(ChangeKind::Module(module))
      }
      Lanes::Priority => {
        let priority = orchester_core::task::organization::Priority::ALL
          .into_iter()
          .find(|priority| lane == super::model::lane_key_of_priority(*priority))?;
        (card.row.organization.priority() != priority).then_some(ChangeKind::Priority(priority))
      }
    }
  }

  fn refuse(&self, card: &CardModel, target: &Target, refusal: Refusal) {
    self.state.refused.set(Some(Refused {
      column: target.column.clone(),
      key: card.row.key.clone(),
      refusal,
    }));
  }

  pub fn subject(&self, card: &CardModel) -> Subject {
    Subject {
      id: card.row.id,
      key: card.row.key.clone(),
      status: card.row.organization.status(),
      shared: self.shared,
    }
  }
}

/// `kinds` as changes of `card`'s task.
fn changes_of(card: &CardModel, kinds: Vec<ChangeKind>) -> Vec<Change> {
  kinds
    .into_iter()
    .map(|kind| Change {
      task: card.row.id,
      key: Arc::clone(&card.row.key),
      kind,
    })
    .collect()
}

/// What a drop of `card` on `target` would do, for a column's look while a drag is over it: refused or not.
pub fn refusal_of(card: &CardModel, target: &Target) -> Option<Refusal> {
  match &target.column {
    ColumnKey::Pipeline(_) => Some(Refusal::Pipeline),
    ColumnKey::Status(status) => match moves::decide(&card.row, *status) {
      Decision::Refused(moves::Refusal::RunNotEnded) => Some(Refusal::RunNotEnded),
      Decision::Refused(moves::Refusal::Incomplete) => Some(Refusal::Incomplete(card.readiness.clone())),
      Decision::Refused(moves::Refusal::HasRun) => Some(Refusal::HasRun),
      Decision::Move(Move::RunAgain) if card.latest_run.is_none() => Some(Refusal::HasRun),
      _ => None,
    },
    ColumnKey::Priority(_) | ColumnKey::Module(_) => None,
  }
}
