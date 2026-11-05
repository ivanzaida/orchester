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
  /// names its `filePath` and whose snippet (`input`) only reads. `None` for any other call, one that may change the
  /// design, and arguments not recorded whole.
  ///
  /// A call with an `edits` patch of a failed snippet is `None` too: Pencil runs the whole patched snippet, and a
  /// fragment that reads only can still move a quote so that the failed snippet's text in a string becomes code.
  pub fn of(server: &str, tool: &str, arguments: Option<&str>) -> Option<Self> {
    if server != SERVER || tool != EXECUTE {
      return None;
    }
    // Arguments that are not JSON were cut at the ceiling of what a request records: not read whole.
    let arguments: Value = serde_json::from_str(arguments?).ok()?;
    if arguments.get("edits").is_some() {
      return None;
    }
    let file = arguments.get("filePath")?.as_str()?.to_owned();
    let exports = script::reads_only(arguments.get("input")?.as_str()?)?;
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
