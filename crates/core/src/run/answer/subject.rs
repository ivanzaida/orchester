//! What a request asks to do, as the answers judge it: a command, classified, or a call of one tool of
//! an MCP server; for a call of one of Orchester's tools that reads outside the worktree, also the folder it reads in,
//! and for `run_check`, the check it runs ([`ToolReach`]).

use std::path::Path;

use crate::{
  native::command::CommandSyntax,
  orchester_tools::{OrchesterTool, ProjectTool, SERVER},
  permission::category::{ClassifiedCommand, CommandScope},
  resource::ApprovalMode,
  run::{CommandShell, NativeCommandAsk, PendingToolApproval},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalSubject {
  Command(ClassifiedCommand),
  McpTool {
    server: String,
    tool: String,
    /// What the call reaches that Approve similar keeps its rule to.
    reach: ToolReach,
  },
}

/// What a call of an MCP tool reaches that Approve similar keeps its rule to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolReach {
  /// Nothing narrower than the tool: a rule covers every call of it.
  Any,
  /// A call of Orchester's tools outside the worktree (`OrchesterTool::reads_outside`), reading in `folder`, as the
  /// request's target names it; `broad` when the folder is a drive or file system root, a home folder, holds one, or
  /// lies fewer than two folders below one: no rule is made for it.
  Folder { folder: String, broad: bool },
  /// A call of `run_check` running the check `name`, as the request's target names it; `every_call` when its access
  /// rules ask before every call, or the check is not known: no rule is made for it.
  Check { name: String, every_call: bool },
}

impl ApprovalSubject {
  /// `approval`'s subject; a command is classified as it runs in `scope`, in the working folder the
  /// request names (Codex reports it; for Claude Code it is the worktree), else in the worktree. A
  /// native agent's command is read with the grammar of what runs it ([`native_command`]).
  pub fn of(approval: &PendingToolApproval, scope: &CommandScope) -> Self {
    if approval.command {
      let folder = approval.target.as_deref().map_or(scope.worktree(), Path::new);
      Self::Command(match approval.native {
        Some(native) => native_command(&approval.action, native, folder, scope),
        None => ClassifiedCommand::classify_in(&approval.action, folder, scope),
      })
    } else {
      let own = (approval.server == SERVER)
        .then(|| OrchesterTool::from_name(&approval.action))
        .flatten();
      let target = approval.target.clone();
      let reach = match (own, target) {
        (Some(tool), Some(folder)) if tool.reads_outside() => ToolReach::Folder {
          broad: scope.is_broad_folder(Path::new(&folder)),
          folder,
        },
        (Some(OrchesterTool::RunCheck), name) => ToolReach::Check {
          name: name.unwrap_or_default(),
          every_call: true,
        },
        _ => ToolReach::Any,
      };
      Self::McpTool {
        server: approval.server.clone(),
        tool: approval.action.clone(),
        reach,
      }
    }
  }

  /// This subject knowing the step's `checks`: a check whose access rules do not ask before every call may be
  /// approved similar.
  #[must_use]
  pub fn with_checks(mut self, checks: &[ProjectTool]) -> Self {
    if let Self::McpTool {
      reach: ToolReach::Check { name, every_call },
      ..
    } = &mut self
    {
      *every_call = checks
        .iter()
        .find(|check| check.name() == name)
        .is_none_or(|check| check.approval() == ApprovalMode::EveryCall);
    }
    self
  }
}

/// A native agent's command `text`: PowerShell's on Windows, zsh's (read as Bash) on macOS, or without a
/// shell the arguments as this platform splits them, which is how the agent's command was started.
fn native_command(text: &str, native: NativeCommandAsk, folder: &Path, scope: &CommandScope) -> ClassifiedCommand {
  match native.shell {
    Some(CommandShell::Pwsh | CommandShell::WindowsPowerShell) => {
      ClassifiedCommand::classify_powershell_in(text, folder, scope)
    }
    Some(CommandShell::Zsh) => ClassifiedCommand::classify_in(text, folder, scope),
    None => match CommandSyntax::HOST.split(text) {
      Ok(argv) => ClassifiedCommand::classify_argv_in(text, &argv, folder, scope),
      Err(_) => ClassifiedCommand::unread(text),
    },
  }
}
