//! `hold_hand_offs`, in debug builds only: holds the review page's hand-offs to the project's runner, so what is kept
//! while one is under way — a question *Sent to Implement*, comments *Sending…* and the tray's *Sending N comments to
//! Implement…* (09i `RBR9O`) — stays on screen until QA captured it (`AGENTS.md` rule 15), then lets them go.

use lurq::mcp::{McpTool, Scope};
use serde_json::{Value, json};

use super::{ToolError, hold_reads::flag, reply};
use crate::project::hold::ReadHold;

const DESCRIPTION: &str = "Debug builds only. `held: true` holds every hand-off to the project's runner that the review \
  page starts from now on (Ask Implement, Ask again, Send to Implement) after the question or the comments are kept, so \
  Sent to Implement, Sending… and the tray's sending state stay on screen. `held: false` lets the waiting hand-offs go \
  to the runner, which takes them or not as it would have. Answers whether hand-offs are held.";

pub fn tool(hold: ReadHold) -> McpTool {
  McpTool::new("hold_hand_offs")
    .description(DESCRIPTION)
    .scope(Scope::Interact)
    .input_schema(json!({
      "type": "object",
      "properties": { "held": { "type": "boolean" } },
      "required": ["held"],
      "additionalProperties": false,
    }))
    .handler(move |_ctx, args| reply(hold_hand_offs(&hold, &args)))
}

/// Holds or releases the hand-offs as `args` say, and answers whether they are held.
pub fn hold_hand_offs(hold: &ReadHold, args: &Value) -> Result<Value, ToolError> {
  hold.set(flag(args)?);
  Ok(json!({ "held": hold.is_held() }))
}
