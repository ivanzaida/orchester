//! A finished step's handoff: its result's summary with what it completed, its decisions, the
//! questions it left open and what the next agent should know. It is kept as task memory once the
//! step's fate is known, written by the step's agent, so the task's write policy decides whether
//! the records are active at once:
//!
//! - a step that passed (for a gated step: once its plan is approved) leaves the handoff and one decision record per
//!   decision;
//! - a step that failed leaves the handoff only, as a failed attempt, and no decisions.
//!
//! One handoff per step of the task stays current: a new one replaces the step's earlier handoff.
//! The latest passing attempt defines the step's decisions, even when it made none: once its
//! handoff is active (at once, or when a person approves it or one of its decisions), the
//! decisions of the step's earlier executions are deprecated. Earlier proposals of the step that no
//! person decided on are withdrawn. A failed attempt's handoff is tagged `failed-attempt` and
//! replaces no decisions.

use std::fmt::Write as _;

use super::{
  AgentProvenance, AgentWrite, Confidence, ContainsSecret, Importance, Memory, MemoryAuthor, MemoryContent, MemoryKind,
  MemoryScope, MemorySettings, MemoryStatus, ScreenedContent, SecretScreen, Tags,
};
use crate::{
  id::TaskId,
  run::{StepOutcome, StepResult},
  time::Timestamp,
};

/// How the step a handoff comes from ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepFate {
  /// It passed, or its plan was approved.
  Passed,
  Failed,
}

/// A finished step's handoff, as its result reported it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handoff {
  summary: String,
  completed: Vec<String>,
  decisions: Vec<String>,
  open_questions: Vec<String>,
  next_agent_should_know: Vec<String>,
}

/// Where a handoff is kept and by whom.
#[derive(Clone, Copy, Debug)]
pub struct HandoffWrite<'a> {
  pub task: TaskId,
  pub provenance: &'a AgentProvenance,
  /// The transcript entries it came from.
  pub sources: &'a [u64],
  pub fate: StepFate,
  pub settings: &'a MemorySettings,
  pub screen: &'a SecretScreen,
  pub now: Timestamp,
}

/// What keeping a handoff changes in task memory: records to add, and earlier ones that change.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryChanges {
  pub added: Vec<Memory>,
  pub changed: Vec<Memory>,
}

/// The heading of the decisions in a handoff's text, which [`super::recall`] leaves out when the
/// decisions reach the prompt as records of their own.
pub(super) const DECISIONS: &str = "Decisions";
/// The headings of a handoff's lists, in the order its text has them.
const LISTS: [&str; 4] = ["Completed", DECISIONS, "Open questions", "The next agent should know"];

/// A handoff record's text split into what comes before its lists (its summary, after the failed
/// attempt's note) and the lists: the lists start at the first paragraph that is one of them.
pub fn split_handoff(text: &str) -> (&str, &str) {
  let mut offset = 0;
  for paragraph in text.split("\n\n") {
    let first_line = paragraph.lines().next().unwrap_or_default();
    if LISTS
      .iter()
      .any(|heading| first_line.strip_suffix(':') == Some(*heading))
    {
      return (text[..offset].trim_end(), &text[offset..]);
    }
    offset += paragraph.len() + 2;
  }
  (text.trim_end(), "")
}

/// The tag of a failed attempt's handoff, which defines none of the step's decisions.
pub const FAILED_ATTEMPT_TAG: &str = "failed-attempt";
const FAILED_ATTEMPT: &str = "This attempt failed; later attempts may take another way.";

impl Handoff {
  /// The handoff of a step that ended with `result`; `None` for a question, which does not end the
  /// step, and for a result that says nothing.
  pub fn from_result(result: &StepResult) -> Option<Self> {
    if result.outcome == StepOutcome::Question {
      return None;
    }
    let handoff = Self {
      summary: result.summary.trim().to_owned(),
      completed: result.completed.clone(),
      decisions: result.decisions.clone(),
      open_questions: result.open_questions.clone(),
      next_agent_should_know: result.next_agent_should_know.clone(),
    };
    (!handoff.text(StepFate::Passed).is_empty()).then_some(handoff)
  }

  /// The handoff record's text: the summary, then each list that has items under its heading. A
  /// failed attempt says so first and lists no decisions.
  pub fn text(&self, fate: StepFate) -> String {
    let mut text = match fate {
      StepFate::Passed => self.summary.clone(),
      StepFate::Failed => format!("{FAILED_ATTEMPT}\n\n{}", self.summary).trim().to_owned(),
    };
    let decisions: &[String] = match fate {
      StepFate::Passed => &self.decisions,
      StepFate::Failed => &[],
    };
    let [completed, _, open_questions, next] = LISTS;
    let lists = [
      (completed, self.completed.as_slice()),
      (DECISIONS, decisions),
      (open_questions, self.open_questions.as_slice()),
      (next, self.next_agent_should_know.as_slice()),
    ];
    for (heading, items) in lists {
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

  /// What keeping the handoff changes, given the task's records now (`earlier`). Refused as a whole
  /// when any new record holds a secret's value.
  pub fn changes(&self, write: &HandoffWrite<'_>, earlier: Vec<Memory>) -> Result<MemoryChanges, ContainsSecret> {
    let mut handoff_content = MemoryContent::new(MemoryKind::Handoff, &self.text(write.fate))
      // `from_result` returns no handoff whose text is blank.
      .expect("a handoff's text is not blank")
      .with_importance(Importance::High);
    if write.fate == StepFate::Failed {
      handoff_content =
        handoff_content.with_tags(Tags::new([FAILED_ATTEMPT_TAG]).expect("the failed-attempt tag is one word"));
    }
    let handoff_content = write.screen.check(handoff_content)?;
    let mut decisions = Vec::new();
    if write.fate == StepFate::Passed {
      for decision in &self.decisions {
        let content = reported(write, MemoryKind::Decision, decision)?;
        let scope = MemoryScope::Task(write.task);
        decisions.push(Memory::by_agent(
          scope,
          agent_write(write),
          content,
          write.settings,
          write.now,
        ));
      }
    }
    let mut step_records: Vec<Memory> = earlier
      .into_iter()
      .filter(|memory| same_step_earlier(memory, write.provenance))
      .collect();
    let current = step_records
      .iter()
      .position(|memory| memory.content().kind() == MemoryKind::Handoff && memory.status() == MemoryStatus::Active);
    let mut handoff = match current {
      Some(index) => Memory::replacing(
        &step_records[index],
        agent_write(write),
        handoff_content,
        write.settings,
        write.now,
      )
      .expect("the step's current handoff is active"),
      None => Memory::by_agent(
        MemoryScope::Task(write.task),
        agent_write(write),
        handoff_content,
        write.settings,
        write.now,
      ),
    };
    let handoff_active = handoff.status() == MemoryStatus::Active;
    // The latest passing attempt defines the step's decisions, even when it made none; a handoff
    // that waits for approval replaces them when it is approved (`super::approval`).
    let replaces_decisions = write.fate == StepFate::Passed && handoff_active;
    let mut changes = MemoryChanges::default();
    for (index, memory) in step_records.iter_mut().enumerate() {
      let changed = match (memory.content().kind(), memory.status()) {
        (MemoryKind::Handoff, MemoryStatus::Active) if Some(index) == current && handoff_active => {
          memory
            .supersede(&mut handoff, write.now)
            .expect("the new handoff is active, of the task, and names the step's current one");
          true
        }
        (MemoryKind::Handoff, MemoryStatus::Proposed) => {
          memory.withdraw(write.now).expect("a proposed record can be withdrawn");
          true
        }
        (MemoryKind::Decision, MemoryStatus::Proposed) if write.fate == StepFate::Passed => {
          memory.withdraw(write.now).expect("a proposed record can be withdrawn");
          true
        }
        (MemoryKind::Decision, MemoryStatus::Active) if replaces_decisions => {
          memory.deprecate(write.now).expect("an active record can be deprecated");
          true
        }
        _ => false,
      };
      if changed {
        changes.changed.push(memory.clone());
      }
    }
    changes.added.push(handoff);
    changes.added.extend(decisions);
    Ok(changes)
  }
}

/// A decision record's content of `kind` saying `text`, of high importance, screened.
fn reported(write: &HandoffWrite<'_>, kind: MemoryKind, text: &str) -> Result<ScreenedContent, ContainsSecret> {
  // The lists' items are not blank: `StepResult` drops blank ones.
  let content = MemoryContent::new(kind, text).expect("handoff list items are not blank");
  write.screen.check(content.with_importance(Importance::High))
}

/// How the step's agent writes: its own report, of medium confidence.
fn agent_write(write: &HandoffWrite<'_>) -> AgentWrite {
  AgentWrite {
    provenance: write.provenance.clone(),
    confidence: Confidence::Medium,
    sources: write.sources.to_vec(),
  }
}

/// Whether `memory` is a handoff or decision an earlier execution of the step wrote.
pub(super) fn same_step_earlier(memory: &Memory, provenance: &AgentProvenance) -> bool {
  matches!(
    memory.author(),
    MemoryAuthor::Agent(agent) if agent.step == provenance.step && agent.execution != provenance.execution
  ) && matches!(memory.content().kind(), MemoryKind::Handoff | MemoryKind::Decision)
}
