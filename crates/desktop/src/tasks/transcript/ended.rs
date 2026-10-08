//! How a command's row says it ended: its exit code when the CLI reported one, and how long it took when both its
//! start and end were seen; never an empty exit code or duration. Claude Code reports no exit codes, so an agent's
//! command that completed without one shows only how long it took, and one that failed reads *failed*. A tool step's
//! command without an exit code reads *no exit code*.

use orchester_core::connection::stream::{CommandStatus, ToolCommandResult};

use super::{CommandEntry, ToolCommandEntry};
use crate::messages::Message;

impl CommandEntry {
  /// What the row's meta says once the command completed or failed; `None` while it runs, when it was declined
  /// (the row says so otherwise), or when it completed with neither an exit code nor a duration to show.
  pub fn ended(&self) -> Option<Message> {
    let failed = match self.status? {
      CommandStatus::Completed => false,
      CommandStatus::Failed => true,
      CommandStatus::Declined => return None,
    };
    let key = match (self.exit_code.is_some(), failed, self.duration.is_some()) {
      (true, _, true) => "runs.activity.exitDuration",
      (true, _, false) => "runs.activity.exit",
      (false, true, true) => "runs.activity.commandFailedDuration",
      (false, true, false) => "runs.activity.commandFailed",
      (false, false, true) => "runs.activity.duration",
      (false, false, false) => return None,
    };
    Some(with_code_and_seconds(key, self.exit_code, self.duration))
  }
}

impl ToolCommandEntry {
  /// What the row's meta says once the program ended; `None` while it runs.
  pub fn ended(&self) -> Option<Message> {
    let (code, millis) = (self.exit_code, self.duration);
    Some(match (self.result?, code, millis) {
      (ToolCommandResult::Stopped, ..) => Message::new("runs.activity.stopped"),
      (ToolCommandResult::TimedOut, _, Some(_)) => with_code_and_seconds("runs.activity.timedOut", None, millis),
      (ToolCommandResult::TimedOut, _, None) => Message::new("runs.activity.timedOutUntimed"),
      (_, Some(_), Some(_)) => with_code_and_seconds("runs.activity.exitDuration", code, millis),
      (_, Some(_), None) => with_code_and_seconds("runs.activity.exit", code, None),
      (_, None, Some(_)) => with_code_and_seconds("runs.activity.noExitCode", None, millis),
      (_, None, None) => Message::new("runs.activity.noExitCodeUntimed"),
    })
  }
}

/// `key` with the placeholders `code` and `seconds` its message has.
fn with_code_and_seconds(key: &'static str, code: Option<i64>, millis: Option<i64>) -> Message {
  let mut message = Message::new(key);
  if let Some(code) = code {
    message = message.with("code", code.to_string());
  }
  if let Some(millis) = millis {
    message = message.with("seconds", tenths_of_seconds(millis));
  }
  message
}

/// `4.2` for 4,213 milliseconds.
fn tenths_of_seconds(millis: i64) -> String {
  let tenths = millis.max(0) / 100;
  format!("{}.{}", tenths / 10, tenths % 10)
}
