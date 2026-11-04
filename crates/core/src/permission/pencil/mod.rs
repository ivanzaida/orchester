//! Reads of a design by the `pencil` MCP server's `execute` tool (ORC-698). A UI step reads the project's design
//! through it (`Get`, `TakeScreenshot`, `Print`, an `Export` of a node to look at), passing the main checkout's design
//! file as `filePath` as `AGENTS.md` asks, so Autopilot could not place it and every call went to the reviewer and on
//! to the person. A call whose snippet only reads ([`script`]) is placed by its file and the files it exports to
//! ([`crate::permission::roots::ToolRequest::Design`]); any other call of the server is not, as before.
//!
//! It lives here, among the rules Autopilot places requests by, not in a rule of the project: a project's rules
//! answer only in Manual and stand for a person's wider answer, while this is what the call touches, as Orchester's
//! own tools are placed.

mod script;

use serde_json::Value;

/// The server's name in an agent profile, as `AGENTS.md` names it.
pub const SERVER: &str = "pencil";

/// The tool that runs a snippet on a design.
const EXECUTE: &str = "execute";

/// A call of the `pencil` server's `execute` that only reads the design `file`, and writes nothing but the files
/// `exports` names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesignRead {
  file: String,
  exports: Vec<String>,
}

impl DesignRead {
  /// The read a call of `server`'s `tool` makes with `arguments`, the JSON its request recorded: an `execute` that
  /// names its `filePath` and whose snippet (`input`), or each fragment its `edits` patch a failed snippet with, only
  /// reads. `None` for any other call, one that may change the design, and arguments not recorded whole.
  ///
  /// A patch is judged by the code it brings: Pencil reverted all the failed snippet did, and that snippet was a
  /// request of the step answered on its own.
  pub fn of(server: &str, tool: &str, arguments: Option<&str>) -> Option<Self> {
    if server != SERVER || tool != EXECUTE {
      return None;
    }
    // Arguments that are not JSON were cut at the ceiling of what a request records: not read whole.
    let arguments: Value = serde_json::from_str(arguments?).ok()?;
    let file = arguments.get("filePath")?.as_str()?.to_owned();
    let (input, edits) = (arguments.get("input"), arguments.get("edits"));
    if input.is_none() && edits.is_none() {
      return None;
    }
    let mut exports = Vec::new();
    if let Some(input) = input {
      exports.extend(script::reads_only(input.as_str()?)?);
    }
    if let Some(edits) = edits {
      for edit in edits.as_array()? {
        exports.extend(script::reads_only(edit.get("replace")?.as_str()?)?);
      }
    }
    Some(Self { file, exports })
  }

  /// The design file, as the call names it.
  pub fn file(&self) -> &str {
    &self.file
  }

  /// The folders and files its `Export`s write, as the call names them.
  pub fn exports(&self) -> &[String] {
    &self.exports
  }
}
