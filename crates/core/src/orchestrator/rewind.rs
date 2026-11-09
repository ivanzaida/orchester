//! Going back in a conversation of the Orchestrator chat (ORC-333, `docs/design/conversation-rewind.md`): *Rewind to
//! here* and *Edit* on an earlier message drop every entry from a point on. What the agent did in what was dropped
//! (tasks created, runs started, settings changed) is not undone: the conversation keeps it on its rewound line
//! ([`ChatEvent::Rewound`]), and the agent is told of it. The agent CLI session that heard the dropped messages is
//! never resumed again: the next turn starts a new one, primed with the conversation as it was kept
//! ([`primed_prompt`]), so the agent goes on as if they had never been said, on Claude Code and Codex alike.

use serde::{Deserialize, Serialize};

use super::{
  chat::{ActionOutcome, ChatAction, ChatEntry, ChatEvent, EntryKind},
  instructions::{report_prompt, wait_prompt, watch_summary_prompt},
  plan::{PlanState, TurnMode},
};
use crate::{id::ChatMessageId, time::Timestamp};

/// Where a rewind goes back to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RewindPoint {
  /// To just before the person's message: it and everything after it go, and its words go back to the composer.
  Before(ChatMessageId),
  /// To just after entry `last`, the end of an answer that began at `answered`: everything after it goes.
  After { last: i64, answered: Timestamp },
}

/// What the rewound line says the conversation went back to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "to", rename_all = "snake_case")]
pub enum RewoundTo {
  /// To before the person's message kept at `at`.
  BeforeMessage { at: Timestamp },
  /// To after the Orchestrator's answer that began at `at`.
  AfterAnswer { at: Timestamp },
}

/// What a rewind from entry `from` on drops: the person's messages, the Orchestrator's answers, and what the agent did
/// in them that stays done.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dropped {
  /// The person's own messages.
  pub mine: u32,
  /// The Orchestrator's answers, one for each turn that said or did something, and its rows of its own: a wait of an
  /// agent and a daily report.
  pub theirs: u32,
  /// What the agent did that stays done, in the order it was done; what earlier rewinds kept among them too.
  pub done: Vec<ChatAction>,
}

impl Dropped {
  /// What dropping the entries of `entries` from seq `from` on drops.
  pub fn of(entries: &[ChatEntry], from: i64) -> Self {
    let mut dropped = Self::default();
    let mut answering = false;
    for entry in entries.iter().filter(|entry| entry.seq >= from) {
      match &entry.kind {
        EntryKind::Person { mode, .. } => {
          answering = false;
          // The turn that carries out a plan has no words of the person's: its answer counts.
          if *mode != TurnMode::CarriesOut {
            dropped.mine += 1;
          }
        }
        EntryKind::Wait { .. } | EntryKind::Report { .. } | EntryKind::WatchEnded { .. } => {
          answering = true;
          dropped.theirs += 1;
        }
        EntryKind::Reply { .. } | EntryKind::Plan(_) => dropped.answer(&mut answering),
        EntryKind::Action(action) => {
          dropped.answer(&mut answering);
          if action.outcome.answered() {
            dropped.done.push(action.clone());
          }
        }
        EntryKind::Event(ChatEvent::Rewound { done, .. }) => dropped.done.extend(done.iter().cloned()),
        EntryKind::Event(_) => {}
      }
    }
    dropped
  }

  /// The messages it drops, the person's and the Orchestrator's.
  pub const fn count(&self) -> u32 {
    self.mine + self.theirs
  }

  /// A part of an answer: the first one of a turn counts the answer.
  const fn answer(&mut self, answering: &mut bool) {
    if !*answering {
      *answering = true;
      self.theirs += 1;
    }
  }
}

/// Whether the conversation that holds `kept` was rewound, or is a fork ([`super::fork`]): its next turn without a
/// session to resume is told the conversation as it was kept ([`primed_prompt`]).
pub fn primes(kept: &[ChatEntry]) -> bool {
  kept.iter().any(|entry| primer(entry).is_some())
}

/// Whether `entry` is a rewind's line (`Some(false)`) or a fork's (`Some(true)`).
const fn primer(entry: &ChatEntry) -> Option<bool> {
  match entry.kind {
    EntryKind::Event(ChatEvent::Rewound { .. }) => Some(false),
    EntryKind::Event(ChatEvent::Forked { .. }) => Some(true),
    _ => None,
  }
}

/// The prompt of the first turn after a rewind, or of a fork: the conversation as it was kept (`kept`, oldest first),
/// what stays done of what was dropped, then the turn's own `prompt`. Only what was kept reaches the new session; a
/// fork's is not told what the original did after the point it was made from.
pub fn primed_prompt(kept: &[ChatEntry], prompt: &str) -> String {
  let forked = kept.iter().rev().find_map(primer).unwrap_or_default();
  let said: Vec<String> = kept.iter().filter_map(said).collect();
  let done: Vec<String> = kept
    .iter()
    .filter_map(|entry| match &entry.kind {
      EntryKind::Event(ChatEvent::Rewound { done, .. }) => Some(done),
      _ => None,
    })
    .flatten()
    .map(action_words)
    .collect();
  let conversation = if said.is_empty() {
    "(nothing: the person went back to its start)".to_owned()
  } else {
    said.join("\n\n")
  };
  let stays = if done.is_empty() {
    String::new()
  } else {
    format!(
      "\n\nWhat you did in the messages the person dropped stays done: it is not undone, so do not do it again, and \
       treat it as existing:\n- {}",
      done.join("\n- ")
    )
  };
  let why = if forked {
    "[Orchester] The person forked an earlier conversation into this new one, copying it up to a point, and this is a \
     new session. What was said after that point in the original is not part of this conversation: go on from the \
     conversation below as if nothing else had been said."
  } else {
    "[Orchester] The person rewound this conversation to an earlier point, and this is a new session. Everything said \
     after that point is gone: go on from the conversation below as if nothing else had been said."
  };
  format!(
    "{why}\n\
     \n\
     The conversation so far, oldest first:\n\
     <conversation>\n{conversation}\n</conversation>{stays}\n\
     \n\
     The person's message now:\n{prompt}"
  )
}

/// What `entry` said, as the primed prompt tells it; `None` for an entry that said nothing to the agent.
fn said(entry: &ChatEntry) -> Option<String> {
  match &entry.kind {
    EntryKind::Person {
      mode: TurnMode::CarriesOut,
      ..
    } => Some("Orchester: the person approved your plan, and you carried it out.".to_owned()),
    EntryKind::Person { text, .. } => Some(format!("The person: {}", text.as_str())),
    EntryKind::Reply { text, .. } => Some(format!("You: {text}")),
    EntryKind::Action(action) => Some(format!("You used a tool: {}", action_words(action))),
    EntryKind::Plan(plan) => {
      let steps: Vec<&str> = plan.steps().iter().map(super::plan::PlanStep::action).collect();
      let state = match plan.state() {
        PlanState::Waits => "it waits for the person",
        PlanState::Approved { .. } => "the person approved it",
        PlanState::Replaced { .. } => "a newer plan replaced it",
        PlanState::Dismissed { .. } => "the person dismissed it",
      };
      Some(format!("You proposed a plan ({state}): {}", steps.join("; ")))
    }
    EntryKind::Wait { wait, .. } => first_line(&wait_prompt(wait)),
    EntryKind::Report { report, .. } => first_line(&report_prompt(report)),
    EntryKind::WatchEnded { summary, .. } => first_line(&watch_summary_prompt(summary)),
    EntryKind::Event(ChatEvent::Rewound { dropped, .. }) => Some(format!(
      "Orchester: the person rewound the conversation here earlier; {dropped} messages were dropped."
    )),
    EntryKind::Event(ChatEvent::Forked { title, .. }) => Some(format!(
      "Orchester: the person forked this conversation from the conversation \"{title}\"; what follows was copied from \
       it."
    )),
    EntryKind::Event(_) => None,
  }
}

fn first_line(text: &str) -> Option<String> {
  text.lines().next().map(str::to_owned)
}

/// What `action` did, in a line: its tool, the task it names, and how it went.
fn action_words(action: &ChatAction) -> String {
  let on = action
    .task
    .as_deref()
    .map(|task| format!(" on {task}"))
    .unwrap_or_default();
  let outcome = match &action.outcome {
    ActionOutcome::Done => "done".to_owned(),
    ActionOutcome::Applied => "the person applied it".to_owned(),
    ActionOutcome::Refused { reason, .. } => format!("refused: {reason}"),
    ActionOutcome::Proposed(_) | ActionOutcome::Waits => "proposed, waiting for the person".to_owned(),
    ActionOutcome::Dismissed => "the person dismissed it".to_owned(),
  };
  format!("{}{on} ({outcome})", action.tool)
}
