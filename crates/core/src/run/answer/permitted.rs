//! What a step's own permissions run without a person although its CLI asked (`docs/plan.md`, "Agent
//! commands without approvals"): Claude Code asks about a command line whose parts it cannot match one
//! by one, such as a chain (`git status && git diff`), a `cd` into the worktree before the command, or
//! a variable the line sets to a literal and uses. When every part would run on its own, listed in the
//! step's allowed commands or a known-safe read inside the worktree
//! ([`CommandPart::known_safe_read`], the user's decision C4), the line runs as its parts would. Nothing
//! else does: a part that is not read to the end, reads a credential, installs, writes outside the
//! worktree, redirects into a file, runs in the background or reads outside the worktree still asks,
//! as it would on its own. The answer is recorded as given by the permissions ([`super::DecidedBy::Permissions`]).

use super::subject::ApprovalSubject;
use crate::permission::{
  AllowedCommands, CommandPermission,
  category::{CommandCategory, CommandPart},
};

/// The category of `subject`, when the step's command permission `commands` runs every part of it on
/// its own; `None` when it asks a person. With *Ask before running commands*, or allowed commands
/// without an entry, every command asks, as the CLI's own rules make it.
pub fn permitted(subject: &ApprovalSubject, commands: &CommandPermission) -> Option<CommandCategory> {
  let (ApprovalSubject::Command(command), CommandPermission::Allowed(list)) = (subject, commands) else {
    return None;
  };
  if list.is_empty() || command.parts().is_empty() {
    return None;
  }
  command
    .parts()
    .iter()
    .all(|part| runs_alone(part, list))
    .then(|| command.category())
}

/// Whether `part` would run without a person on its own: a known-safe read inside the worktree, or a
/// listed command that names nothing outside the worktree and redirects into no file.
fn runs_alone(part: &CommandPart, list: &AllowedCommands) -> bool {
  part.known_safe_read()
    || (list.allows_argv(part.words())
      && matches!(part.category(), CommandCategory::ReadOnly | CommandCategory::Worktree)
      && !part.always_asks()
      && !part.installs()
      && !part.reaches_outside()
      && !part.writes_output())
}
