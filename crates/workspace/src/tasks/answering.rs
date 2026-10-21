//! Answers sent from this window that the project's runner has not confirmed or refused yet (ORC-70): the one rule
//! that makes an answered item leave every *Needs you* at once. Sending an answer marks the exact waiting item it
//! answers ([`Answered`]) in its project's [`PendingAnswers`]; [`super::data::ProjectTasks::with_answered`] then
//! leaves it out of what waits, so the pill, the panel, the Overview, a tab's mark and the badge drop it and count
//! one less in the same frame. A refusal (a stale item, a person gate, the runner down) takes the mark back and the
//! item returns, the reason shown where the person answered. A confirmation keeps it marked until a reading of the
//! project's tasks begun after it, which no longer lists the item, takes over.

use std::sync::Arc;

use orchester_core::{
  id::RunId,
  run::PlanVersion,
};
use orchester_ipc::Command;

use super::{
  conversation::outgoing::Action,
  needs_you::{WaitingItem, WaitingKind},
};

/// The waiting item an answer answers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answered {
  /// A tool request of `run`, by its id.
  Approval { run: RunId, request: Arc<str> },
  /// A question with suggested answers of `run`, by its request's id.
  Choice { run: RunId, request: Arc<str> },
  /// Plan `version` of `run`.
  Plan { run: RunId, version: PlanVersion },
  /// The decision `run` waits for after its step's retries.
  Decision { run: RunId },
  /// The limit `run` stopped at.
  Limit { run: RunId },
  /// The agents' questions of `run` that its conversation answers: a message the composer sent as their answer.
  Message { run: RunId },
}

impl Answered {
  /// What `command` answers, if it answers an item that waits for a person.
  pub fn of(command: &Command) -> Option<Self> {
    Some(match command {
      Command::DecideToolApproval { run, approval, .. } => Self::Approval {
        run: *run,
        request: approval.as_str().into(),
      },
      Command::AnswerQuestion { run, request, .. } => Self::Choice {
        run: *run,
        request: request.as_str().into(),
      },
      Command::DecidePlan { run, version, .. } => Self::Plan {
        run: *run,
        version: *version,
      },
      Command::DecideStuckRun { run, .. } | Command::DecideStuckRunAs { run, .. } => Self::Decision { run: *run },
      Command::RaiseLimit { run, .. } | Command::RaiseStepBudget { run, .. } => Self::Limit { run: *run },
      _ => return None,
    })
  }

  /// What a composer's send `action` to `run` answers, if anything: the agent's question, the plan's changes, or the
  /// decision a continuation takes.
  pub const fn of_action(action: Action, run: RunId) -> Option<Self> {
    match action {
      Action::Answer(_) => Some(Self::Message { run }),
      Action::PlanChanges(version) => Some(Self::Plan { run, version }),
      Action::Continue => Some(Self::Decision { run }),
      _ => None,
    }
  }

  /// Whether this answers `item`.
  pub fn covers(&self, item: &WaitingItem) -> bool {
    let of = |run: &RunId| item.run.is_some_and(|(id, _)| id == *run);
    match (self, &item.kind) {
      (Self::Approval { run, request }, WaitingKind::Approval(shown)) => {
        shown.approval.run == *run && shown.approval.id == **request
      }
      (Self::Choice { run, request }, WaitingKind::Choice(choice)) => of(run) && choice.request == *request,
      (Self::Plan { run, version }, WaitingKind::Plan(plan)) => of(run) && plan.version == *version,
      (Self::Decision { run }, WaitingKind::Decision(_))
      | (Self::Limit { run }, WaitingKind::StoppedAtLimit { .. })
      | (Self::Message { run }, WaitingKind::Question(_)) => of(run),
      _ => false,
    }
  }
}

/// Where a pending answer stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
  /// On its way to the runner.
  Sent,
  /// The runner took it, or what became of it is not known; settled by a reading begun once `0` answers were.
  Settled(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Pending {
  id: u64,
  answered: Answered,
  stage: Stage,
}

/// One project's answers sent from this window and not yet read back from its tasks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PendingAnswers {
  pending: Arc<Vec<Pending>>,
  /// Counts the answers the runner confirmed or refused, or that ended without word.
  settled: u64,
  /// The id the next mark takes.
  next: u64,
}

impl PendingAnswers {
  /// How many answers have settled: the project's tasks are read again whenever it moves, and a reading begun at
  /// it ends the marks settled by then ([`Self::read_at`]).
  pub const fn settled(&self) -> u64 {
    self.settled
  }

  /// What the answers still marked answer.
  pub fn answered(&self) -> Vec<Answered> {
    self.pending.iter().map(|pending| pending.answered.clone()).collect()
  }

  /// Whether nothing is marked.
  pub fn is_empty(&self) -> bool {
    self.pending.is_empty()
  }

  /// Marks `answered` as answered; the mark's id.
  pub(crate) fn mark(&mut self, answered: Answered) -> u64 {
    let id = self.next;
    self.next += 1;
    Arc::make_mut(&mut self.pending).push(Pending {
      id,
      answered,
      stage: Stage::Sent,
    });
    id
  }

  /// The answer marked `id` settled: `taken` keeps it marked until the next reading, a refusal takes it back now.
  pub(crate) fn settle(&mut self, id: u64, taken: bool) {
    self.settled += 1;
    let settled = self.settled;
    let pending = Arc::make_mut(&mut self.pending);
    if taken {
      for mark in pending.iter_mut().filter(|mark| mark.id == id) {
        mark.stage = Stage::Settled(settled);
      }
    } else {
      pending.retain(|mark| mark.id != id);
    }
  }

  /// A reading of the tasks begun once `settled` answers had settled ended: it tells what became of those, so their
  /// marks go. Whether any went.
  pub fn read_at(&mut self, settled: u64) -> bool {
    let read = |mark: &Pending| matches!(mark.stage, Stage::Settled(at) if at <= settled);
    if !self.pending.iter().any(read) {
      return false;
    }
    Arc::make_mut(&mut self.pending).retain(|mark| !read(mark));
    true
  }
}
