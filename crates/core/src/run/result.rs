//! What a step left behind for the steps after it. An agent ends a new session with a structured
//! final message (Codex `--output-schema`, `.tmp/cli-probe/REPORT.md`: with `--json` the final
//! agent message is JSON matching the schema): whether it did the step, or asks the person a
//! question before it can go on, and a summary — the question, when it asks one. When the final
//! message is not such a result (a continued session, which is started without the schema, or a
//! CLI that ignored it), the CLI's exit decides and the message itself is the output.
//!
//! Claude Code gets the same schema (`--json-schema`) and reports the result apart from its text
//! (`structured_output`). When it does not, its final message is searched with
//! [`StepResult::at_end`]: the result must close the message, so free text is never taken for a
//! question. A Claude Code turn that ends with no result at all fails
//! (`crate::connection::stream::ClaudeStream`).

use serde::{Deserialize, Serialize};

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
      "description":"done: current step complete and required checks pass. A completed review or verification with a negative verdict or unresolved blocking or major findings is failed, even if its report is complete. question: waiting for the person's decision, permission, access, or clarification; resumes the same session after their answer. failed: failure handled by the pipeline, including checks requiring corrections by an earlier step, or failure that no decision or action by the person would let the step resolve. Incomplete work or a permission request alone is not failed."
    },
    "summary":{
      "type":"string",
      "description":"For question, explain the blocker and saved progress and ask a concrete question with available options. Otherwise summarize the completed work or failure."
    }
  },
  "required":["outcome","summary"],
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
  /// It needs the person's decision, permission, access, or clarification; the summary asks for it.
  Question,
}

/// A final message that follows [`STEP_RESULT_SCHEMA`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepResult {
  pub outcome: StepOutcome,
  pub summary: String,
}

impl StepResult {
  /// The structured result `message` holds, if it is one.
  pub fn parse(message: &str) -> Option<Self> {
    serde_json::from_str(message.trim()).ok()
  }

  /// The result that closes `message`: the whole message, else a fenced code block that ends it,
  /// else a JSON object that starts a line and runs to the end. Anything else after it, or a
  /// document with other fields, is no result.
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
