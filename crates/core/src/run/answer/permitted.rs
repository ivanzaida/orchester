//! What a step's own permissions run without a person although its CLI asked (`docs/plan.md`, "Agent
//! commands without approvals"): Claude Code asks about a command line whose parts it cannot match one
//! by one, such as a chain (`git status && git diff`), a `cd` into the worktree before the command, or
//! a variable the line sets to a literal and uses. When every part would run on its own, listed in the
//! step's allowed commands or a known-safe read inside the worktree
//! ([`CommandPart::known_safe_read`], the user's decision C4), the line runs as its parts would. In a
//! step that edits files, a part that only writes files in the worktree or its writable folders as the
//! edit tools would ([`CommandPart::edits_worktree`]: `sed -i`, `cat > f <<'EOF'`, `tee`, `mkdir`, `cp`,
//! `rm`, a redirect into a file) runs too (the user's decision of 2026-10-05): the edit tools write
//! there without asking. Nothing else does: a part that is not read to the end, reads a credential,
//! installs, writes outside the worktree or where no edit may write, runs in the background or reads
//! outside the worktree still asks, as it would on its own; in a read-only step every write asks. The answer is
//! recorded as given by the permissions ([`super::DecidedBy::Permissions`]).

use super::subject::ApprovalSubject;
use crate::permission::{
  AgentPermissions, AllowedCommands, CommandPermission, RepositoryAccess,
  category::{CommandCategory, CommandPart},
};

/// The category of `subject`, when the step's effective `permissions` run every part of it on its own;
/// `None` when it asks a person. With *Ask before running commands*, or allowed commands without an
/// entry, every command asks, as the CLI's own rules make it.
pub fn permitted(subject: &ApprovalSubject, permissions: &AgentPermissions) -> Option<CommandCategory> {
  let (ApprovalSubject::Command(command), CommandPermission::Allowed(list)) = (subject, permissions.commands()) else {
    return None;
  };
  if list.is_empty() || command.parts().is_empty() {
    return None;
  }
  let edits = permissions.repository() == RepositoryAccess::Write;
  command
    .parts()
    .iter()
    .all(|part| runs_alone(part, list, edits))
    .then(|| command.category())
}

/// Whether `part` would run without a person on its own: a known-safe read inside the worktree, a
/// listed command that names nothing outside the worktree, or, when the step `edits`, a part that only
/// writes there as an edit would. A listed command redirects into a file only when the step edits.
fn runs_alone(part: &CommandPart, list: &AllowedCommands, edits: bool) -> bool {
  if part.through_path() {
    return false;
  }
  part.known_safe_read()
    || (edits && part.edits_worktree())
    || (list.allows_argv(part.words())
      && matches!(part.category(), CommandCategory::ReadOnly | CommandCategory::Worktree)
      && !part.always_asks()
      && !part.installs()
      && !part.reaches_outside()
      && (edits || !part.writes_output()))
}
