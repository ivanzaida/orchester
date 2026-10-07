//! Orchester's `ask_question` tool for Claude Code, served by the runner's relay beside Orchester's tools
//! (`crate::orchester_tools`). Claude Code's own `AskUserQuestion` is never offered: `--tools` lists only the
//! tools a step's permissions need (`crate::permission::claude`). The call waits in the agent's turn; the
//! person's answer is its result, and the agent goes on in the same turn. It takes the native agent's
//! `ask_question` input, with options required: a plain question ends the turn as the step's result.

use serde_json::{Value, json};

use super::Question;
use crate::{
  native::tools::{ToolInput, ToolName, parse_call},
  orchester_tools::SERVER,
};

/// The tool's name, as the relay lists it.
pub const NAME: &str = "ask_question";

const DESCRIPTION: &str = "Ask the person to choose between options you suggest, when you need their decision to \
                           continue. Give at least two options; they may pick one (or several with multi_select) or \
                           answer in their own words. Your turn waits for the answer, which comes back as this \
                           tool's result.";

/// What a Claude Code agent's prompt says of the tool.
pub const CLAUDE_PROMPT: &str = "When you need the person to decide between options before you can go on, call \
                                 mcp__Orchester__ask_question with the question and the options. Their answer comes \
                                 back as its result, and you continue. A question without options ends your turn \
                                 with the `question` outcome instead.";

/// The tool as the relay's `tools/list` answer lists it.
pub fn listed() -> Value {
  let schema: Value =
    serde_json::from_str(ToolName::AskQuestion.input_schema()).expect("the static tool schema is valid JSON");
  json!({
    "name": NAME,
    "description": DESCRIPTION,
    "inputSchema": schema,
    "annotations": {"readOnlyHint": true, "destructiveHint": false, "openWorldHint": false},
  })
}

/// The Claude Code allow rule that lets the agent call the tool without its permission prompt.
pub fn claude_allow_rule() -> String {
  format!("mcp__{SERVER}__{NAME}")
}

/// The question a call with `arguments` asks; the error result the agent gets when it is invalid or
/// suggests no options.
pub fn parse(arguments: &Value) -> Result<Question, String> {
  let input = parse_call(NAME, arguments).map_err(|error| error.to_string())?;
  let ToolInput::AskQuestion(ask) = input else {
    return Err(format!("`{NAME}` is not a question"));
  };
  ask.structured().ok_or_else(|| {
    format!("`{NAME}` needs at least two options; to ask without options, end your turn with the `question` outcome")
  })
}
