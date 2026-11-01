//! A command that Codex's sandbox kept from starting because the sandbox itself could not be set up (issue #69).
//!
//! On Windows, before a command of its elevated sandbox, Codex 0.161.0 has its setup helper
//! (`codex-windows-sandbox-setup.exe`) refresh the sandbox's setup. When that failed, the command did not start, and
//! the agent of issue #69 was told the helper's error as `<code>: <message>` (`helper_unknown_error: setup refresh had
//! errors`), the `code` and `message` Codex also wrote to `.sandbox\setup_error.json` in its home folder; its sandbox
//! log there names the file the helper could not change. Every later command failed the same way, read-only or
//! writing, so the stream reports the failure once ([`super::AppServerStream`]).

use serde_json::Value;

use crate::connection::stream::RunnerProblem;

/// The prefix of the codes of Codex's sandbox setup helper.
const HELPER_CODE: &str = "helper_";

/// The setup failure a command item that did not complete reports in its output, if any.
pub(super) fn setup_failure(item: &Value) -> Option<RunnerProblem> {
  if item.get("status").and_then(Value::as_str) == Some("completed") {
    return None;
  }
  let output = item.get("aggregatedOutput").and_then(Value::as_str)?;
  output.lines().find_map(helper_error)
}

/// `<code>: <message>` in `line`, where `code` is one of the helper's codes, a whole word, and `message` names a stage
/// of the setup. The message is kept whole, to its line's end: a cut could keep part of a secret value, which the
/// runner's redaction of the whole value would then miss.
fn helper_error(line: &str) -> Option<RunnerProblem> {
  line.match_indices(HELPER_CODE).find_map(|(start, _)| {
    let before = line[..start].chars().next_back();
    if before.is_some_and(|before| before.is_ascii_alphanumeric() || before == '_') {
      return None;
    }
    let rest = &line[start..];
    let end = rest
      .find(|letter: char| !(letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '_'))
      .unwrap_or(rest.len());
    let (code, message) = rest.split_at(end);
    let message = message.strip_prefix(": ")?.trim();
    if code.len() == HELPER_CODE.len() || !message.to_ascii_lowercase().contains("setup") {
      return None;
    }
    Some(RunnerProblem::CodexSandboxSetupFailed {
      code: code.to_owned(),
      message: message.to_owned(),
    })
  })
}
