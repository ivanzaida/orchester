use super::{AgentEvent, MemoryNotSavedReason, RunnerProblem};
use crate::connection::mcp::McpServerProblem;

impl AgentEvent {
  /// Redacts displayed and persisted content while preserving protocol correlation identifiers.
  pub fn redact(&mut self, redact: &impl Fn(&str) -> String) {
    match self {
      Self::Message { text } | Self::Reasoning { text } => *text = redact(text),
      Self::CommandStarted { command, .. }
      | Self::PermissionDenied { command, .. }
      | Self::ToolCommandStarted { command, .. } => *command = redact(command),
      Self::CommandFinished { command, output, .. } | Self::ToolCommandFinished { command, output, .. } => {
        *command = redact(command);
        *output = redact(output);
      }
      Self::Error { message } | Self::TurnFailed { message } | Self::NotSignedIn { message } => {
        *message = redact(message);
      }
      Self::Raw { line, .. } => *line = redact(line),
      Self::FileChanged { changes, .. } => {
        for change in changes {
          change.path = redact(&change.path);
        }
      }
      Self::RunnerProblem { problem } => problem.redact(redact),
      Self::McpServer { server, problem } => {
        if let Some(server) = server {
          *server = redact(server);
        }
        problem.redact(redact);
      }
      Self::ToolDecided { action, decision, .. } => {
        *action = redact(action);
        if let Some(told) = &mut decision.told {
          *told = redact(told);
        }
      }
      // A tool name the server reported.
      Self::McpToolDeclined { tool, .. } | Self::McpToolFinished { tool, .. } => *tool = redact(tool),
      // Servers and tools named as the project configured them; plan limits are numbers and the CLI's plan name.
      Self::PlanLimits { .. }
      | Self::SessionStarted { .. }
      | Self::TurnStarted
      | Self::TurnCompleted { .. }
      | Self::McpToolNotAsked { .. }
      | Self::McpExemptionNotApplied { .. } => {}
    }
  }
}

impl McpServerProblem {
  fn redact(&mut self, redact: &impl Fn(&str) -> String) {
    match self {
      Self::FailedToStart { detail: Some(detail) } => *detail = redact(detail),
      Self::ProgramNotFound { program } => *program = redact(program),
      Self::SecretMissing { secret } | Self::SecretArchived { secret } | Self::SecretUnreadable { secret } => {
        *secret = redact(secret);
      }
      Self::SharedVariable { variable } | Self::ReservedVariable { variable } => *variable = redact(variable),
      Self::Missing
      | Self::Disabled
      | Self::WorkingDirectoryUnsupported
      | Self::UnsafeScriptArgument { .. }
      | Self::NotUnicode
      | Self::FailedToStart { detail: None } => {}
    }
  }
}

impl RunnerProblem {
  fn redact(&mut self, redact: &impl Fn(&str) -> String) {
    match self {
      Self::CouldNotStart { detail }
      | Self::CheckpointFailed { detail }
      | Self::CodebaseIndexUnavailable { detail }
      | Self::RipgrepUnavailable { detail, .. }
      | Self::MemoryScanFailed { detail } => *detail = redact(detail),
      Self::CodexSettingsIgnored { folder } => *folder = redact(folder),
      Self::UnavailableVariable { variable } => *variable = redact(variable),
      Self::ProgramNotFound { program } => *program = redact(program),
      Self::MergeConflict { branch, files } => {
        *branch = redact(branch);
        for path in files {
          *path = redact(path);
        }
      }
      Self::MemoryNotSaved { reason } => reason.redact(redact),
      // Versions, a model id and counts, parsed as such: no text that could hold a secret.
      Self::StepBudgetReached { .. }
      | Self::RipgrepNotFound { .. }
      | Self::MemoryUnreadable { .. }
      | Self::StderrLeftOut { .. }
      | Self::CliTooOld { .. }
      | Self::PermissionPromptUnreachable => {}
    }
  }
}

impl MemoryNotSavedReason {
  fn redact(&mut self, redact: &impl Fn(&str) -> String) {
    match self {
      Self::ContainsSecret { secret } | Self::KeychainUnavailable { secret } => *secret = redact(secret),
      Self::StorageFailed { detail } => *detail = redact(detail),
    }
  }
}
