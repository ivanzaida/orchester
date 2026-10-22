//! What the project's Orchestrator chat wrote to a run's agents, as a task's conversation shows it (ORC-45). A message
//! or an answer it wrote is its own, never the person's: it names the chat's conversation it came from and how it went
//! out, sent by the chat while the person's switch let it, or proposed by the chat and applied by the person
//! ([`orchester_core::orchestrator::FromOrchestrator`]). So is a decision on how a run that stopped goes on
//! (`continue_run`: one more retry, a return with a note, a return to resolve conflicts): the return or restart it
//! started is the chat's ([`continued`]). An answer or a message it proposed and no one applied yet
//! waits for the person in the conversation too, with the same proposal behind it in the chat: applying or dismissing
//! it in either place decides both ([`crate::orchestrator::proposal::decide`]). One applying would refuse now is
//! stale: a message the run would no longer take as it was checked (`send_message`'s own check), or an answer to a
//! question answered meanwhile. A dismissed one, or one refused when it was applied, stays as a muted line. None of
//! them ever reads as sent.

use std::sync::Arc;

use orchester_core::{
  id::{ChatId, RunId},
  orchestrator::{
    ActionOutcome, FromOrchestrator, SentWay,
    change::{Change, MessageThen},
  },
  question::AskedQuestions,
  time::Timestamp,
};
use orchester_store::KeptAction;

use super::{
  composer::ComposerMode,
  outgoing::{conversation_of, offer},
};
use crate::{
  app_mcp::{SendTiming, sent_as},
  tasks::{
    detail::TaskDetail,
    timeline::{Item, ItemKind},
  },
};

/// What a proposal would send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Proposed {
  /// A message to the run's agents, and what it does to a run none of whose agents waits or works (`then`: runs the
  /// task again, continues it, ...), as it was checked.
  Message { text: Arc<str>, then: Option<MessageThen> },
  /// The answer to the question with suggested answers `request`: the options picked and/or the chat's own words.
  Answer {
    request: Arc<str>,
    choices: Vec<Arc<str>>,
    text: Option<Arc<str>>,
  },
}

/// Where a proposal stands, as the conversation shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposalState {
  /// It waits for the person: *Apply* or *Dismiss*.
  Waits,
  /// A person's decision took it and is being carried out.
  Applying,
  /// The person dismissed it: nothing was sent.
  Dismissed,
  /// It still waits in the chat, but applying it would be refused: the run changed since it was checked, or its
  /// question was answered.
  Stale,
  /// Applying it was refused, for `0`, in the words the chat keeps: nothing was sent.
  Refused(Arc<str>),
}

/// An answer or a message the chat proposed for the task's latest run, and no one applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
  /// The chat's conversation and entry that hold the proposal: what *Apply* and *Dismiss* decide.
  pub chat: ChatId,
  pub seq: i64,
  /// When the chat proposed it.
  pub at: Timestamp,
  pub proposed: Proposed,
  pub state: ProposalState,
}

impl Proposal {
  /// Whether it waits for the person's decision.
  pub fn waits(&self) -> bool {
    self.state == ProposalState::Waits
  }

  /// Stable while the conversation grows.
  pub fn key(&self) -> Arc<str> {
    format!("proposal-{}-{}", self.chat, self.seq).into()
  }
}

/// What a message sent now to `run` of the task `detail` holds would meet: the composer's mode of the task's latest
/// run, as `send_message` reads it; `None` once `run` is not the latest, where every message is refused.
pub fn message_offer(detail: &TaskDetail, run: RunId) -> Option<ComposerMode> {
  let latest = detail.run.as_ref().filter(|latest| latest.id() == run)?;
  Some(offer(&conversation_of(detail, false), Some(latest), None).0)
}

/// The proposals among `kept`, the chat's actions that answer the run's agents or send them a message, oldest first:
/// what was sent (by the chat or applied by the person) shows as its message or answer, and what the chat's own call
/// refused was never proposed, so neither is one. `offered`: what a message sent now would meet
/// ([`message_offer`]); the run's `questions` tell which still wait for an answer.
pub fn proposals(
  kept: &[KeptAction],
  (offered, questions): (Option<&ComposerMode>, &[AskedQuestions]),
) -> Vec<Proposal> {
  kept
    .iter()
    .filter_map(|kept| {
      let change = &kept.action.change.as_ref()?.change;
      let proposed = proposed(change)?;
      let action = &kept.action;
      let state = match &action.outcome {
        ActionOutcome::Waits if action.claimed.is_some() => ProposalState::Applying,
        ActionOutcome::Waits if is_stale(change, (offered, questions)) => ProposalState::Stale,
        ActionOutcome::Waits => ProposalState::Waits,
        ActionOutcome::Dismissed => ProposalState::Dismissed,
        ActionOutcome::Refused { reason, .. } if action.settled.is_some() => {
          ProposalState::Refused(reason.as_str().into())
        }
        _ => return None,
      };
      Some(Proposal {
        chat: kept.chat,
        seq: kept.seq,
        at: kept.at,
        proposed,
        state,
      })
    })
    .collect()
}

/// The proposal that waits to answer the question `request`, if one does.
pub fn waiting_answer<'a>(proposals: &'a [Proposal], request: &str) -> Option<&'a Proposal> {
  proposals.iter().find(|proposal| {
    proposal.waits() && matches!(&proposal.proposed, Proposed::Answer { request: asked, .. } if &**asked == request)
  })
}

/// Names as the Orchestrator's each return or restart among `items` that a decision of the chat on how the run goes on
/// started (`continue_run`, among `kept`): a decision carried out, by the chat itself or applied by the person, on the
/// attempt the run stopped in starts the next one. A proposal no one applied, or one refused, started nothing.
pub fn continued(items: &mut [Item], kept: &[KeptAction]) {
  for item in items {
    if let ItemKind::Mark(mark) = &mut item.kind {
      mark.orchestrator = continuation(kept, mark.attempt);
    }
  }
}

/// The chat's decision among `kept` that started attempt `attempt`, with its conversation and how it went out.
fn continuation(kept: &[KeptAction], attempt: u32) -> Option<FromOrchestrator> {
  kept.iter().rev().find_map(|kept| {
    let Change::Continue { attempt: stopped, .. } = kept.action.change.as_ref()?.change else {
      return None;
    };
    let way = match kept.action.outcome {
      ActionOutcome::Applied => SentWay::AppliedByPerson,
      ActionOutcome::Done => SentWay::Direct,
      _ => return None,
    };
    (stopped.checked_add(1) == Some(attempt)).then(|| FromOrchestrator::new(kept.chat, way))
  })
}

fn proposed(change: &Change) -> Option<Proposed> {
  match change {
    Change::Message { text, then, .. } => Some(Proposed::Message {
      text: text.as_str().into(),
      then: *then,
    }),
    Change::Answer {
      request, choices, text, ..
    } => Some(Proposed::Answer {
      request: request.as_str().into(),
      choices: choices.iter().map(|choice| choice.as_str().into()).collect(),
      text: text.as_deref().map(Arc::from),
    }),
    Change::Publish { .. }
    | Change::Cancel { .. }
    | Change::Merge { .. }
    | Change::Continue { .. }
    | Change::Pause { .. }
    | Change::Resume { .. }
    | Change::Release { .. }
    | Change::InitRepository { .. } => None,
  }
}

/// Whether applying `change` now would be refused: a message that would no longer do what it was checked as (or was
/// kept before that was recorded), as `send_message` checks it; an answer whose question no longer waits.
fn is_stale(change: &Change, (offered, questions): (Option<&ComposerMode>, &[AskedQuestions])) -> bool {
  match change {
    Change::Message {
      now, sent_as: checked, ..
    } => {
      let timing = if *now { SendTiming::Now } else { SendTiming::AfterTurn };
      let would = offered.map(|mode| sent_as(mode, Some(timing)));
      checked.is_none() || would != checked.as_deref()
    }
    Change::Answer { request, .. } => !questions
      .iter()
      .any(|asked| asked.request.id() == request && asked.waits()),
    _ => false,
  }
}
