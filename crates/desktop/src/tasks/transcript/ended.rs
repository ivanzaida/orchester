//! How a command's row says it ended: its exit code when the CLI reported one, and how long it took when both its
//! start and end were seen. A CLI that reports no exit code (Claude Code never does) is not shown an empty one: a
//! failed command reads *failed*, one that completed *no exit code*, as a tool step's command does.

use orchester_core::connection::stream::CommandStatus;

use super::CommandEntry;
use crate::messages::Message;

impl CommandEntry {
  /// What the row's meta says once the command completed or failed; `None` while it runs or when it was declined,
  /// which the row says otherwise.
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
      (false, false, true) => "runs.activity.noExitCode",
      (false, false, false) => "runs.activity.noExitCodeUntimed",
    };
    let mut message = Message::new(key);
    if let Some(code) = self.exit_code {
      message = message.with("code", code.to_string());
    }
    if let Some(millis) = self.duration {
      message = message.with("seconds", tenths_of_seconds(millis));
    }
    Some(message)
  }
}

/// `4.2` for 4,213 milliseconds.
pub fn tenths_of_seconds(millis: i64) -> String {
  let tenths = millis.max(0) / 100;
  format!("{}.{}", tenths / 10, tenths % 10)
}
