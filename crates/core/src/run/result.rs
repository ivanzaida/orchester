//! What a step left behind for the steps after it. An agent ends a new session with a structured
//! final message (Codex: the turn's `outputSchema`; the final agent message is JSON matching the
//! schema, `.tmp/cli-probe/REPORT.md`): whether it did the step, or asks the person a
//! question before it can go on, and a summary — the question, when it asks one. When the final
//! message is not such a result (a continued session, which is started without the schema, or a
//! CLI that ignored it), the CLI's exit decides and the message itself is the output.
//!
//! Claude Code gets the same schema (`--json-schema`) and reports the result apart from its text
//! (`structured_output`). When it does not, its final message is searched with
//! [`StepResult::at_end`]: the result must close the message, so free text is never taken for a
//! question. A Claude Code turn that ends with no result at all fails
//! (`crate::connection::stream::ClaudeStream`). A result's `outcome` and `summary` are read
//! strictly; fields the schema does not name are tolerated.
//!
//! A step that ran but could not check part of what it is for (a state it could not reach, an app or tool it could
//! not use) reports `unverified` and lists each such check with why ([`UnverifiedCheck`]): no verdict on the work, so
//! the run waits for the person instead of taking the step's failure route.
//!
//! A result may also carry a handoff for the agents after it (what it completed, its decisions, the
//! questions it left open and what the next agent should know), read leniently and kept by the
//! runner as task memory ([`crate::memory::Handoff`]).

use serde::{Deserialize, Deserializer, Serialize};

use super::status::StepFailure;
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
      "enum":["done","failed","unverified","question"],
      "description":"done: current step complete and required checks pass. A completed review or verification with a negative verdict or unresolved blocking or major findings is failed, even if its report is complete. unverified: the step ran, but some of what it must check could not be checked (a state it could not reach, an app, tool or fixture it could not use); list each in unverified with why. It is no verdict on the work: the person decides, and nothing returns to an earlier step. Mismatches it did find make the step failed, not unverified. question: waiting for the person's decision, permission, access, or clarification; resumes the same session after their answer. failed: failure handled by the pipeline, including checks requiring corrections by an earlier step, or failure that no decision or action by the person would let the step resolve. Incomplete work or a permission request alone is not failed."
    },
    "summary":{
      "type":"string",
      "description":"For question, explain the blocker and saved progress and ask a concrete question with available options. Otherwise summarize the completed work or failure."
    },
    "unverified":{
      "type":["array","null"],
      "items":{
        "type":"object",
        "properties":{
          "check":{"type":"string","description":"What could not be checked, such as a design state or frame."},
          "reason":{"type":"string","description":"Why it could not be checked."}
        },
        "required":["check","reason"],
        "additionalProperties":false
      },
      "description":"For unverified: each check that could not be done, with why; null otherwise."
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
  "required":["outcome","summary","unverified","completed","decisions","open_questions","next_agent_should_know"],
  "additionalProperties":false
}"#;

/// Whether the agent's work passed the step, failed its checks, or needs a person.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepOutcome {
  /// The current step is complete and its required checks pass.
  Done,
  /// Failed checks needing an earlier step's corrections, or an unresolvable step failure.
  Failed,
  /// The step ran but could not check everything it is for; [`StepResult::unverified`] says what and why.
  Unverified,
  /// It needs the person's decision, permission, access, or clarification; the summary asks for it.
  Question,
}

/// A final message that follows [`STEP_RESULT_SCHEMA`]. `outcome` and `summary` are read strictly;
/// the handoff is read leniently, so a malformed handoff never costs the step its outcome: a missing
/// or `null` list is empty, a single string is one item, items that are not strings or are blank are
/// dropped, and fields the schema does not name are ignored. The schema lists the handoff fields as
/// required but nullable, since Codex's structured output requires every property.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepResult {
  pub outcome: StepOutcome,
  pub summary: String,
  #[serde(default, deserialize_with = "checks")]
  pub unverified: Vec<UnverifiedCheck>,
  #[serde(default, deserialize_with = "list")]
  pub completed: Vec<String>,
  #[serde(default, deserialize_with = "list")]
  pub decisions: Vec<String>,
  #[serde(default, deserialize_with = "list")]
  pub open_questions: Vec<String>,
  #[serde(default, deserialize_with = "list")]
  pub next_agent_should_know: Vec<String>,
}

/// One check an `unverified` step could not do, and why.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UnverifiedCheck {
  pub check: String,
  pub reason: String,
}

/// The unverified checks, read from whatever JSON the agent wrote for them as leniently as a handoff list: an item
/// that is no object with a non-blank `check` is dropped, and a missing `reason` is empty.
fn checks<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<UnverifiedCheck>, D::Error> {
  let serde_json::Value::Array(items) = serde_json::Value::deserialize(deserializer)? else {
    return Ok(Vec::new());
  };
  let text = |item: &serde_json::Value, field: &str| {
    item
      .get(field)
      .and_then(serde_json::Value::as_str)
      .map_or_else(String::new, |text| text.trim().to_owned())
  };
  Ok(
    items
      .iter()
      .map(|item| UnverifiedCheck {
        check: text(item, "check"),
        reason: text(item, "reason"),
      })
      .filter(|item| !item.check.is_empty())
      .collect(),
  )
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

  /// The result that closes `message`: the whole message, else a fenced code block that ends it,
  /// else a JSON object that starts a line and runs to the end. Anything else after it, or an
  /// object without a valid `outcome` and `summary`, is no result.
  pub fn at_end(message: &str) -> Option<Self> {
    let message = message.trim_end();
    Self::parse(message)
      .or_else(|| Self::fenced_at_end(message))
      .or_else(|| Self::object_at_end(message))
  }

  fn fenced_at_end(message: &str) -> Option<Self> {
    let body = message.strip_suffix(FENCE)?;
    let (_, block) = body.rsplit_once(FENCE)?;
    // The opening fence's line may name the language (`json`).
    let (language, json) = block.split_once('\n')?;
    if !language.trim().chars().all(|c| c.is_ascii_alphanumeric()) {
      return None;
    }
    Self::parse(json)
  }

  fn object_at_end(message: &str) -> Option<Self> {
    message
      .match_indices('\n')
      .map(|(newline, _)| &message[newline + 1..])
      .filter(|line| line.trim_start().starts_with('{'))
      .find_map(Self::parse)
  }
}

/// A Markdown code fence.
const FENCE: &str = "```";

/// What one step execution produced: the summary of its structured result, else its final message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepOutput {
  pub run: RunId,
  pub execution: StepExecutionId,
  pub step: StepKey,
  /// `None` when the final message was not a structured result.
  pub outcome: Option<StepOutcome>,
  pub text: String,
  /// What an `unverified` result could not check; empty for any other.
  pub unverified: Vec<UnverifiedCheck>,
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
    let (outcome, text, unverified) = match result {
      Some(result) if result.outcome == StepOutcome::Unverified => {
        (Some(result.outcome), result.summary, result.unverified)
      }
      Some(result) => (Some(result.outcome), result.summary, Vec::new()),
      None => (None, message.unwrap_or_default().trim().to_owned(), Vec::new()),
    };
    Self {
      run,
      execution,
      step,
      outcome,
      text,
      unverified,
      created_at: now,
    }
  }

  /// Why the step of an agent that ended with this output fails, `None` when it does not;
  /// `turn_succeeded`: the CLI completed its turn and exited cleanly. A `failed` result decides,
  /// also when the turn failed after the agent delivered it: it is a verdict on the work even when a
  /// permission prompt closed afterwards; an `unverified` result decides the same way, for the person.
  /// Any other failed turn is a breakage. A `done` result does not outweigh it: the turn may have
  /// failed because nothing could be approved, and a step never passes as if a person had been asked.
  pub const fn failure(&self, turn_succeeded: bool) -> Option<StepFailure> {
    match self.outcome {
      Some(StepOutcome::Failed) => Some(StepFailure::ReportedFailed),
      Some(StepOutcome::Unverified) => Some(StepFailure::Unverified),
      _ if turn_succeeded => None,
      _ => Some(StepFailure::AgentFailed),
    }
  }
}
