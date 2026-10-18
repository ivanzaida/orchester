//! What the project's Orchestrator chat wrote to a run's agents, as a task's conversation shows it (ORC-45). A message
//! or an answer it wrote is its own, never the person's: it names the chat's conversation it came from and how it went
//! out, sent by the chat while the person's switch let it, or proposed by the chat and applied by the person
//! ([`orchester_core::orchestrator::FromOrchestrator`]). An answer or a message it proposed and no one applied yet
//! waits for the person in the conversation too, with the same proposal behind it in the chat: applying or dismissing
//! it in either place decides both ([`crate::orchestrator::proposal::decide`]). One the run no longer takes, or whose
//! question was answered meanwhile, is stale; a dismissed one, or one refused when it was applied, stays as a muted
//! line. None of them ever reads as sent.

use std::sync::Arc;

use orchester_core::{
  id::ChatId,
  orchestrator::{ActionOutcome, change::Change},
  question::AskedQuestions,
  time::Timestamp,
};
use orchester_store::KeptAction;

/// What a proposal would send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Proposed {
  /// A message to the run's agents.
  Message { text: Arc<str> },
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
  /// It still waits in the chat, but the run no longer takes it, or its question was answered: applying it is refused.
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

/// The proposals among `kept`, the chat's actions that answer the run's agents or send them a message, oldest first:
/// what was sent (by the chat or applied by the person) shows as its message or answer, and what the chat's own call
/// refused was never proposed, so neither is one. `takes_messages`: an agent of the run still reads a message; the
/// run's `questions` tell which still wait for an answer.
pub fn proposals(kept: &[KeptAction], (takes_messages, questions): (bool, &[AskedQuestions])) -> Vec<Proposal> {
  kept
    .iter()
    .filter_map(|kept| {
      let proposed = proposed(&kept.action.change.as_ref()?.change)?;
      let action = &kept.action;
      let state = match &action.outcome {
        ActionOutcome::Waits if action.claimed.is_some() => ProposalState::Applying,
        ActionOutcome::Waits if is_stale(&proposed, (takes_messages, questions)) => ProposalState::Stale,
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

fn proposed(change: &Change) -> Option<Proposed> {
  match change {
    Change::Message { text, .. } => Some(Proposed::Message {
      text: text.as_str().into(),
    }),
    Change::Answer {
      request, choices, text, ..
    } => Some(Proposed::Answer {
      request: request.as_str().into(),
      choices: choices.iter().map(|choice| choice.as_str().into()).collect(),
      text: text.as_deref().map(Arc::from),
    }),
    Change::Publish { .. } | Change::Cancel { .. } | Change::Merge { .. } => None,
  }
}

/// Whether applying `proposed` now would be refused: no agent reads a message any more, or its question no longer
/// waits.
fn is_stale(proposed: &Proposed, (takes_messages, questions): (bool, &[AskedQuestions])) -> bool {
  match proposed {
    Proposed::Message { .. } => !takes_messages,
    Proposed::Answer { request, .. } => !questions
      .iter()
      .any(|asked| asked.request.id() == &**request && asked.waits()),
  }
}
