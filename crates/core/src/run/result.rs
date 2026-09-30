//! What a step left behind for the steps after it. An agent ends a new session with a structured
//! final message (Codex `--output-schema`, `.tmp/cli-probe/REPORT.md`: with `--json` the final
//! agent message is JSON matching the schema): whether it did the step, or asks the person a
//! question before it can go on, and a summary — the question, when it asks one. When the final
//! message is not such a result (a continued session, which is started without the schema, or a
//! CLI that ignored it), the CLI's exit decides and the message itself is the output.
//!
//! Claude Code runs without a schema; its prompt asks for the same result as its final message,
//! which is read the same way. Only a final message that is exactly such a JSON document counts, so
//! free text is never taken for a question; a question written as prose is read as the step's
//! output.
//!
//! A result may also carry a handoff for the agents after it (what it completed, its decisions, the
//! questions it left open and what the next agent should know), which the runner keeps as task
//! memory ([`crate::memory::Handoff`]).

use serde::{Deserialize, Deserializer, Serialize};

use crate::{
  id::{RunId, StepExecutionId},
  resource::StepKey,
  time::Timestamp,
};

/// The JSON schema of a step's final message.
pub const STEP_RESULT_SCHEMA: &str = r#"{
  "type":"object",
  "properties":{
    "outcome":{
      "type":"string",
      "enum":["done","failed","question"],
      "description":"done: current step complete. question: waiting for the person's decision, permission, access, or clarification; resumes the same session after their answer. failed: failure handled by the pipeline, no decision or action by the person would let the step continue. Incomplete work or a permission request alone is not failed."
    },
    "summary":{
      "type":"string",
      "description":"For question, explain the blocker and saved progress and ask a concrete question with available options. Otherwise summarize the completed work or failure."
    },
    "completed":{
      "type":["array","null"],
      "items":{"type":"string"},
      "description":"Handoff: what this step completed, one item each; null when nothing is worth listing."
    },
    "decisions":{
      "type":["array","null"],
      "items":{"type":"string"},
      "description":"Handoff: decisions made in this step that later steps must follow, one per item."
    },
    "open_questions":{
      "type":["array","null"],
      "items":{"type":"string"},
      "description":"Handoff: questions this step left open."
    },
    "next_agent_should_know":{
      "type":["array","null"],
      "items":{"type":"string"},
      "description":"Handoff: what the next agent should know before it starts, such as pitfalls or where things are."
    }
  },
  "required":["outcome","summary","completed","decisions","open_questions","next_agent_should_know"],
  "additionalProperties":false
}"#;

/// Whether the agent says it did the step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepOutcome {
  /// The current step is complete.
  Done,
  /// Failure handled by the pipeline; no decision or action by the person would let the step continue.
  Failed,
  /// It needs the person's decision, permission, access, or clarification; the summary asks for it.
  Question,
}

/// A final message that follows [`STEP_RESULT_SCHEMA`]. `outcome` and `summary` are read strictly;
/// the handoff is read leniently, so a malformed handoff never costs the step its outcome: a missing
/// or `null` list is empty, a single string is one item, items that are not strings or are blank are
/// dropped, and fields the schema does not name are ignored. The schema lists the handoff fields as
/// required but nullable, since Codex's structured output requires every property.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct StepResult {
  pub outcome: StepOutcome,
  pub summary: String,
  #[serde(default, deserialize_with = "list")]
  pub completed: Vec<String>,
  #[serde(default, deserialize_with = "list")]
  pub decisions: Vec<String>,
  #[serde(default, deserialize_with = "list")]
  pub open_questions: Vec<String>,
  #[serde(default, deserialize_with = "list")]
  pub next_agent_should_know: Vec<String>,
}

/// A handoff list, read from whatever JSON the agent wrote for it.
fn list<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
  let items = match serde_json::Value::deserialize(deserializer)? {
    serde_json::Value::String(item) => vec![item],
    serde_json::Value::Array(items) => items
      .into_iter()
      .filter_map(|item| match item {
        serde_json::Value::String(item) => Some(item),
        _ => None,
      })
      .collect(),
    _ => Vec::new(),
  };
  Ok(
    items
      .into_iter()
      .map(|item| item.trim().to_owned())
      .filter(|item| !item.is_empty())
      .collect(),
  )
}

impl StepResult {
  /// The structured result `message` holds, if it is one.
  pub fn parse(message: &str) -> Option<Self> {
    serde_json::from_str(message.trim()).ok()
  }
}

/// What one step execution produced: the summary of its structured result, else its final message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepOutput {
  pub run: RunId,
  pub execution: StepExecutionId,
  pub step: StepKey,
  /// `None` when the final message was not a structured result.
  pub outcome: Option<StepOutcome>,
  pub text: String,
  pub created_at: Timestamp,
}

impl StepOutput {
  /// The output of `execution` from its agent's final message (`None`: it wrote none).
  pub fn from_final_message(
    run: RunId,
    execution: StepExecutionId,
    step: StepKey,
    message: Option<&str>,
    now: Timestamp,
  ) -> Self {
    let result = message.and_then(StepResult::parse);
    let (outcome, text) = match result {
      Some(result) => (Some(result.outcome), result.summary),
      None => (None, message.unwrap_or_default().trim().to_owned()),
    };
    Self {
      run,
      execution,
      step,
      outcome,
      text,
      created_at: now,
    }
  }
}
