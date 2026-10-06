use serde::{Deserialize, Serialize};

/// Why an agent works without an MCP server its profile chooses. The runner leaves a server out
/// before the CLI starts when it cannot be given to it as configured, and the CLI reports one that
/// did not start; either way the run's activity says so, and the agent works on without it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "problem", rename_all = "snake_case")]
pub enum McpServerProblem {
  /// The profile chooses a server the project no longer has.
  Missing,
  /// The server is turned off in Connections.
  Disabled,
  /// Its program is no file on the `PATH` or in the worktree.
  ProgramNotFound { program: String },
  /// Claude Code starts every server in the worktree; this one is set to start in another folder.
  WorkingDirectoryUnsupported,
  /// Windows: the program is a command script, and argument `position` (from 0) would be changed by
  /// `cmd.exe` on its way to the program.
  UnsafeScriptArgument { position: usize },
  /// A path of the server is not valid Unicode, so it cannot be written into the CLI's configuration.
  NotUnicode,
  /// A value comes from secret `secret` (its id), which the project no longer has.
  SecretMissing { secret: String },
  /// A value comes from secret `secret` (its name), which is archived.
  SecretArchived { secret: String },
  /// The keychain did not give the value of secret `secret` (its name): it refused, or has no value for it on
  /// this computer. The runner's log says why.
  SecretUnreadable { secret: String },
  /// Another server of the same agent takes `variable` from a different secret. Codex hands both to
  /// its servers from its own environment, by name, so only one value can reach them.
  SharedVariable { variable: String },
  /// `variable` is one the agent CLI itself starts with; Codex hands a server's secret variables over
  /// from its own environment, where this one would replace the CLI's.
  ReservedVariable { variable: String },
  /// The agent's CLI reported that the server did not start; `detail` is what it said, if anything.
  FailedToStart { detail: Option<String> },
  /// A native agent reaches no MCP server yet (`docs/architecture/native-agent.md` §5.4, P4).
  NotSupportedByConnection,
}
