//! An agent of the chat's project waits for a person (`docs/design/orchestrator-toolset.md`, "Decisions" 5): it asked
//! a question with suggested answers, its step waits for a message, or its plan waits for a decision; or a task's
//! latest run ended or stopped ([`RunEnd`]). The Orchestrator's conversation keeps one entry per wait, linking to its
//! card, in the conversation that last dealt with its task ([`dealt_with`]); with *Let the Orchestrator answer
//! agents* on, the wait also starts one Orchestrator turn by itself, unless the Orchestrator's own answer, message or
//! decision raised it, so the chat and an agent never answer each other in a loop.

use serde::{Deserialize, Serialize};

use super::chat::{ChatEntry, EntryKind};
use crate::{
  id::{ChatId, RunId},
  run::{Run, RunLimit, RunStatus, Stuck},
};

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
  /// Its task's latest run ended or stopped so: the person decides what comes next.
  Ended(RunEnd),
}

impl WaitKind {
  /// How the run ended or stopped, for a run's end; `None` for an agent's wait.
  pub const fn run_end(self) -> Option<RunEnd> {
    match self {
      Self::Ended(end) => Some(end),
      Self::Question | Self::Message | Self::Plan => None,
    }
  }
}

/// How a task's latest run ended or stopped, as the chat follows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "end", rename_all = "snake_case")]
pub enum RunEnd {
  /// It succeeded: its work is ready to merge.
  Succeeded,
  Failed,
  /// A step used up its retries, broke again, or waits unverified or unchanged (`why`): the run waits for a person's
  /// decision on how it goes on.
  Stopped {
    why: StopReason,
  },
  /// It stopped at `limit`: its token budget, time limit, the daily or group budget, a step's budget or an account's
  /// plan limit.
  Limit {
    limit: RunLimit,
  },
  /// It stopped without settling, and goes on only when a person continues it.
  Interrupted,
}

/// Why a run waits for a person's decision on how it goes on ([`Stuck`]), as people read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum StopReason {
  /// Its step used up the `retries` its failure route allows.
  RetriesExhausted { retries: u32 },
  /// Its step broke again in the attempt that restarted it.
  BrokeAgain,
  /// Its step could not check everything it is for.
  Unverified,
  /// Its step was not run again on work that has not changed.
  Unchanged,
}

impl StopReason {
  /// Why `run` waits for a person's decision, its step's failure route allowing the retries it names; `None` for a
  /// run that does not wait for one.
  pub fn of_run(run: &Run) -> Option<Self> {
    let RunStatus::WaitingForPerson(stuck) = run.status() else {
      return None;
    };
    Some(match stuck {
      Stuck::RetriesExhausted(step) => Self::RetriesExhausted {
        retries: run
          .snapshot()
          .body()
          .step(step)
          .and_then(|step| step.on_failure.as_ref())
          .map_or(0, |route| route.max_retries.get()),
      },
      Stuck::BrokeAgain(_) => Self::BrokeAgain,
      Stuck::Unverified(_) => Self::Unverified,
      Stuck::Unchanged(_) => Self::Unchanged,
    })
  }
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
  /// The step's place in the run's pipeline, from 1, when a run's end names its step.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub place: Option<u32>,
  /// How many other runs the same limit stopped at once, kept in this one entry (the daily or a group's budget).
  #[serde(default, skip_serializing_if = "is_zero")]
  pub others: u32,
  /// The waiting agent's role, when it is known.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub agent: Option<String>,
  /// The app link of its card: the task's Conversation; for a run's end, the task's Summary.
  pub link: String,
  /// The Orchestrator's own answer, message or decision on how the run continues raised it ([`raised_by_own`]).
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub own: bool,
  /// It starts an Orchestrator turn by itself: *Let the Orchestrator answer agents* was on when it was kept, and the
  /// Orchestrator's own answer did not raise it.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub automatic: bool,
}

/// Whether `wait` now was raised by the Orchestrator's own doing, so that it starts no turn by itself: in `entries`,
/// those of every conversation in the order they were kept (the person may have started another since), the newest
/// entry about its run, before a wait of the run, is an answer, a message or a decision on how the run continues that
/// the chat gave (applied, or done itself), or a resume of the run the chat did; for a run's end also a start of the
/// run the chat took itself. A run that succeeded is never the chat's own: its turn only tells the person and
/// proposes, so it cannot answer the chat in a loop. An agent the Orchestrator answered that asks again, or a run it
/// continued, resumed or started that stops, is answered by a person, or by an Orchestrator turn the person starts.
pub fn raised_by_own(entries: &[ChatEntry], wait: &AgentWait) -> bool {
  let run = wait.run;
  let end = wait.kind.run_end();
  if end == Some(RunEnd::Succeeded) {
    return false;
  }
  for entry in entries.iter().rev() {
    match &entry.kind {
      EntryKind::Wait { wait: kept, .. } if kept.run == run => return false,
      EntryKind::Action(action) if action.outcome.answered() => {
        let carried = action
          .change
          .as_ref()
          .is_some_and(|change| change.change.carries_run_on() && change.change.run() == Some(run));
        let started = end.is_some() && action.started == Some(run);
        if carried || action.resumed == Some(run) || started {
          return true;
        }
      }
      _ => {}
    }
  }
  false
}

const fn is_zero(count: &u32) -> bool {
  *count == 0
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
