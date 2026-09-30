//! A finished step's handoff: its result's summary with what it completed, its decisions, the
//! questions it left open and what the next agent should know. The runner keeps it as task memory
//! — the handoff as one record and each decision as a record of its own — written by the step's
//! agent, so the task's write policy decides whether they are active at once.

use std::fmt::Write as _;

use super::{
  AgentProvenance, AgentWrite, Confidence, ContainsSecret, Importance, Memory, MemoryContent, MemoryKind, MemoryScope,
  MemorySettings, SecretScreen,
};
use crate::{
  id::TaskId,
  run::{StepOutcome, StepResult},
  time::Timestamp,
};

/// The records a finished step's result leaves, before they are checked for secrets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handoff {
  handoff: MemoryContent,
  decisions: Vec<MemoryContent>,
}

impl Handoff {
  /// The handoff of a step that finished with `result`; `None` for a question, which does not
  /// finish the step, and for a result that says nothing.
  pub fn from_result(result: &StepResult) -> Option<Self> {
    if result.outcome == StepOutcome::Question {
      return None;
    }
    let reported =
      |kind, text: &str| MemoryContent::new(kind, text).map(|content| content.with_importance(Importance::High));
    // Only a blank text is refused: a result that says nothing leaves no handoff.
    let handoff = reported(MemoryKind::Handoff, &text(result)).ok()?;
    let decisions = result
      .decisions
      .iter()
      // A blank decision is no decision.
      .filter_map(|decision| reported(MemoryKind::Decision, decision).ok())
      .collect();
    Some(Self { handoff, decisions })
  }

  /// The handoff record's text.
  pub fn text(&self) -> &str {
    self.handoff.text()
  }

  /// The task memory records of the handoff, written by the agent of `provenance` as the project's
  /// `settings` keep task memory, of medium confidence (the agent's own report, unchecked);
  /// `sources` are the transcript entries it came from. Refused as a whole when any of them holds a
  /// secret's value.
  pub fn records(
    &self,
    task: TaskId,
    (provenance, sources): (&AgentProvenance, &[u64]),
    (settings, screen): (&MemorySettings, &SecretScreen),
    now: Timestamp,
  ) -> Result<Vec<Memory>, ContainsSecret> {
    std::iter::once(&self.handoff)
      .chain(&self.decisions)
      .map(|content| {
        let write = AgentWrite {
          provenance: provenance.clone(),
          confidence: Confidence::Medium,
          sources: sources.to_vec(),
        };
        let screened = screen.check(content.clone())?;
        Ok(Memory::by_agent(
          MemoryScope::Task(task),
          write,
          screened,
          settings,
          now,
        ))
      })
      .collect()
  }
}

/// The summary, then each list that has items under its heading.
fn text(result: &StepResult) -> String {
  let mut text = result.summary.trim().to_owned();
  let lists = [
    ("Completed", &result.completed),
    ("Decisions", &result.decisions),
    ("Open questions", &result.open_questions),
    ("The next agent should know", &result.next_agent_should_know),
  ];
  for (heading, items) in lists {
    let items: Vec<&str> = items
      .iter()
      .map(|item| item.trim())
      .filter(|item| !item.is_empty())
      .collect();
    if items.is_empty() {
      continue;
    }
    if !text.is_empty() {
      text.push_str("\n\n");
    }
    text.push_str(heading);
    text.push(':');
    for item in items {
      // Writing to a String cannot fail.
      let _ = write!(text, "\n- {item}");
    }
  }
  text
}
