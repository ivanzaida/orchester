//! What the agent is answered: a program's exit code and its output, cut to a size, or why the call did
//! not run. The answer is an MCP `tools/call` result ([`ToolResult::mcp`]).

use std::{fmt::Write, time::Duration};

use serde_json::{Value, json};

use super::ToolCallError;
use crate::{
  command::{CommandError, SecretVariable, redact_secrets},
  resource::ExitCodes,
};

/// Which end of a program's output is kept when it is longer than its limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputKept {
  /// The start, where a diff or a log begins.
  Head,
  /// The end, where a build or a test run reports how it went, as a tool step keeps it.
  Tail,
}

/// What a program left, as the caller's process runner captured it within the program's cap
/// ([`super::ToolProgram::cap`]): its exit code, the kept part of each stream, and how many bytes of them
/// were not kept. A plain record: the runner reads the streams as they come, never holding more than the cap.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProgramOutput {
  /// `None` for a program ended by a signal (macOS).
  pub code: Option<i32>,
  pub stdout: String,
  pub stderr: String,
  pub left_out: u64,
}

/// The answer to one call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolResult {
  text: String,
  is_error: bool,
}

impl ToolResult {
  /// The answer of a call that ran and has `text` to show.
  pub(super) const fn shown(text: String) -> Self {
    Self { text, is_error: false }
  }

  /// The answer of a call that did not run.
  pub fn refused(error: &ToolCallError) -> Self {
    Self {
      text: error.to_string(),
      is_error: true,
    }
  }

  /// The answer of a file tool: its text, an error when `is_error`.
  pub const fn of_file_tool(text: String, is_error: bool) -> Self {
    Self { text, is_error }
  }

  /// The answer of a call that could not reach its program, such as Orchester's tools being unavailable.
  pub fn unavailable(reason: &str) -> Self {
    Self {
      text: reason.to_owned(),
      is_error: true,
    }
  }

  /// The answer of a program that ended as `outcome` says: its exit code, then what it printed, cut to
  /// `limit` bytes at the `kept` end. An exit code outside `expected`, a stop at `timeout` or a start
  /// that failed is an error result.
  pub(super) fn of_program(
    outcome: Result<ProgramOutput, CommandError>,
    expected: &ExitCodes,
    (limit, kept): (usize, OutputKept),
    timeout: Duration,
  ) -> Self {
    let output = match outcome {
      Ok(output) => output,
      Err(CommandError::TimedOut) => {
        return Self {
          text: format!(
            "the program did not finish within {} seconds and was stopped",
            timeout.as_secs()
          ),
          is_error: true,
        };
      }
      Err(error) => {
        return Self {
          text: error.to_string(),
          is_error: true,
        };
      }
    };
    let code = output
      .code
      .map_or_else(|| "none (ended by a signal)".to_owned(), |code| code.to_string());
    let mut printed = output.stdout;
    if !output.stderr.is_empty() {
      if !printed.is_empty() && !printed.ends_with('\n') {
        printed.push('\n');
      }
      printed.push_str("[stderr]\n");
      printed.push_str(&output.stderr);
    }
    let (shown, cut) = bounded(&printed, limit, kept);
    let left_out = output.left_out + cut as u64;
    let mut text = format!("exit code: {code}\n{shown}");
    if left_out > 0 {
      let end = match kept {
        OutputKept::Head => "end",
        OutputKept::Tail => "start",
      };
      // Writing to a String cannot fail.
      let _ = write!(text, "\n[{left_out} bytes of output at the {end} left out]");
    }
    Self {
      text,
      is_error: !output.code.is_some_and(|code| expected.contains(i64::from(code))),
    }
  }

  pub fn text(&self) -> &str {
    &self.text
  }

  pub const fn is_error(&self) -> bool {
    self.is_error
  }

  /// This answer with the values of `secrets` removed.
  #[must_use]
  pub fn redacted(self, secrets: &[SecretVariable]) -> Self {
    Self {
      text: redact_secrets(&self.text, secrets),
      is_error: self.is_error,
    }
  }

  /// The MCP `tools/call` result.
  pub fn mcp(&self) -> Value {
    json!({"content": [{"type": "text", "text": self.text}], "isError": self.is_error})
  }
}

/// `text` cut to at most `limit` bytes at the `kept` end, on a character boundary, and how many bytes
/// were left out.
fn bounded(text: &str, limit: usize, kept: OutputKept) -> (&str, usize) {
  if text.len() <= limit {
    return (text, 0);
  }
  let shown = match kept {
    OutputKept::Head => {
      let mut end = limit;
      while !text.is_char_boundary(end) {
        end -= 1;
      }
      &text[..end]
    }
    OutputKept::Tail => {
      let mut start = text.len() - limit;
      while !text.is_char_boundary(start) {
        start += 1;
      }
      &text[start..]
    }
  };
  (shown, text.len() - shown.len())
}
