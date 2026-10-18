//! An agent of the chat's project waits for a person (`docs/design/orchestrator-toolset.md`, "Decisions" 5): it asked
//! a question with suggested answers, its step waits for a message, or its plan waits for a decision. The
//! Orchestrator's conversation keeps one entry per wait, linking to its card, in the conversation that last dealt
//! with its task ([`dealt_with`]); with *Let the Orchestrator answer agents* on, the wait also starts one Orchestrator
//! turn by itself, unless the Orchestrator's own answer raised it, so the chat and an agent never answer each other in
//! a loop.

use serde::{Deserialize, Serialize};

use super::chat::{ChatEntry, EntryKind};
use crate::id::{ChatId, RunId};

/// What an agent waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitKind {
  /// The answer to its question with suggested answers.
  Question,
  /// A message: the answer to the question it wrote in the conversation.
  Message,
  /// A decision on its plan.
  Plan,
}

/// One wait, as the conversation keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWait {
  pub kind: WaitKind,
  /// The task's key as people read it (`KON-41`), and its title.
  pub task: String,
  pub title: String,
  pub run: RunId,
  pub number: u32,
  /// The waiting step's name, while the run's pipeline has it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub step: Option<String>,
  /// The waiting agent's role, when it is known.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub agent: Option<String>,
  /// The app link of its card: the task's Conversation.
  pub link: String,
  /// The Orchestrator's own answer or message to the run raised it.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub own: bool,
  /// It starts an Orchestrator turn by itself: *Let the Orchestrator answer agents* was on when it was kept, and the
  /// Orchestrator's own answer did not raise it.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub automatic: bool,
}

/// Whether a wait of `run` now was raised by the Orchestrator's own answer: in `entries`, those of every conversation
/// in the order they were kept (the person may have started another since), the newest entry about `run` is an
/// answer or message the chat gave its agents (applied, or sent itself), not a wait. An agent the Orchestrator
/// answered that asks again is answered by a person, or by an Orchestrator turn the person starts.
pub fn raised_by_own(entries: &[ChatEntry], run: RunId) -> bool {
  for entry in entries.iter().rev() {
    match &entry.kind {
      EntryKind::Wait { wait, .. } if wait.run == run => return false,
      EntryKind::Action(action) => {
        let answered = action
          .change
          .as_ref()
          .is_some_and(|change| change.change.answers_agents() && change.change.run() == Some(run));
        if answered && action.outcome.answered() {
          return true;
        }
      }
      _ => {}
    }
  }
  false
}

/// The conversation that last dealt with `wait`'s task, among `entries`, those of every conversation, each with the
/// conversation it was kept in: the one whose newest entry about the task or the waiting run is the newest of all
/// (by `seq`, the order entries were kept in). An entry is about them when it is an action the Orchestrator took on
/// the task (named by its key, in any case) or on the run (an answer, a message, a cancel or a merge of it, or its
/// start), whether it was done, proposed, refused or dismissed, or a wait of the task kept before. `None` when no
/// conversation dealt with it: the wait then goes to the project's latest conversation.
pub fn dealt_with<'a>(entries: impl IntoIterator<Item = (ChatId, &'a ChatEntry)>, wait: &AgentWait) -> Option<ChatId> {
  entries
    .into_iter()
    .filter(|(_, entry)| is_about(&entry.kind, wait))
    .max_by_key(|(_, entry)| entry.seq)
    .map(|(chat, _)| chat)
}

/// Whether `kind` is about `wait`'s task or run.
fn is_about(kind: &EntryKind, wait: &AgentWait) -> bool {
  let task = |key: &str| key.eq_ignore_ascii_case(&wait.task);
  match kind {
    EntryKind::Wait { wait: kept, .. } => kept.run == wait.run || task(&kept.task),
    EntryKind::Action(action) => {
      action.task.as_deref().is_some_and(task)
        || action.started == Some(wait.run)
        || action.change.as_ref().and_then(|change| change.change.run()) == Some(wait.run)
    }
    EntryKind::Person { .. } | EntryKind::Reply { .. } | EntryKind::Event(_) => false,
  }
}
