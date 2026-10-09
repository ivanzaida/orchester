//! `hold_reads`, in debug builds only: holds the reads of the project feeds, so a screen's loading state
//! stays on screen until QA captured it (`AGENTS.md` rule 15), then lets them go.

use lurq::mcp::{McpTool, Scope};
use serde_json::{Value, json};

use super::{ToolError, object, reply};
use crate::project::hold::ReadHold;

const DESCRIPTION: &str = "Debug builds only. `held: true` holds every read of a project's tasks and limits that \
  starts from now on, so a screen that reads them for the first time stays in its loading state (open a project \
  no tab showed yet, e.g. with navigate); data already on screen stays while a read waits. `held: false` lets \
  the waiting reads go. Answers whether reads are held.";

pub fn tool(hold: ReadHold) -> McpTool {
  McpTool::new("hold_reads")
    .description(DESCRIPTION)
    .scope(Scope::Interact)
    .input_schema(json!({
      "type": "object",
      "properties": { "held": { "type": "boolean" } },
      "required": ["held"],
      "additionalProperties": false,
    }))
    .handler(move |_ctx, args| reply(hold_reads(&hold, &args)))
}

/// Holds or releases the reads as `args` say, and answers whether they are held.
pub fn hold_reads(hold: &ReadHold, args: &Value) -> Result<Value, ToolError> {
  let held = match object(args)?.get("held") {
    Some(Value::Bool(held)) => *held,
    Some(_) => return Err(ToolError::NotABool { param: "held" }),
    None => return Err(ToolError::Missing { param: "held" }),
  };
  hold.set(held);
  Ok(json!({ "held": hold.is_held() }))
}
