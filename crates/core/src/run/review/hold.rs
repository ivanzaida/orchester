//! The hard rules of auto mode: what goes to the person without a review, whatever a reviewer would say.
//! Every part of a command line Orchester can read is checked on its own, so one harmless part never hides
//! another: a part that reads a credential, rewrites history or deletes outside the worktree, pushes
//! anything but the run's own branch to its `origin` ([`super::push`]), writes in a read-only step, may reach the
//! network while it is off, or writes outside the worktree and the step's writable folders sends the
//! whole line to the person.
//!
//! In a read-only step a Cargo build, check, test, clippy, doc or bench whose only write is its build output (the
//! person's decision of 2026-10-18) is no write: its target directory ([`CommandPart::build_output`]) lies in a
//! writable folder outside the worktree, or in the worktree where its repository ignores it and tracks nothing in it
//! ([`HoldRules::build_output`]). Such a build goes to the reviewer; every other hard rule still applies to it, and
//! `cargo fmt`, `fix`, `add`, `install` or `publish`, an alias, what after `--` may write, a build pointed at a
//! manifest or target directory elsewhere, and a target directory the repository does not ignore still hold. The
//! person accepted (2026-10-18) that such a build runs the worktree's own code, whatever an earlier step wrote, under
//! the reviewer's approval alone: build scripts, procedural macros, the tests, and the repository's
//! `.cargo/config.toml` (`runner`, `rustc-wrapper`, `rustflags`) and `rust-toolchain.toml`; and that a project rule
//! for `start_qa_app` lets agents run code they wrote outside every sandbox, pinned only by the script's blob.
//!
//! A command that writes nothing is no write in a read-only step either ([`CommandPart::writes_nothing`]): a read,
//! and a lookup over the network that keeps nothing (`cargo search`, `git ls-remote`, `git remote show`). It goes to
//! the reviewer; the network rule still holds it while the step's network is off, and a redirect into a file, an
//! alias, or a manifest elsewhere still hold. A write to a tool's own cache outside the worktree counts as a write
//! here (the person's decision of 2026-10-23): a lookup or read that may refresh Cargo's registry index or downloads,
//! npm's or pip's cache, or bring `Cargo.lock` up to date (`cargo info`, `cargo tree` without `--frozen`, `npm
//! view`, `pip index versions`) holds in a read-only step. For the outside-folders rule it does not count: in a step
//! that may write, such a lookup goes to the reviewer, as a build that fills the same caches does.
//!
//! A part Orchester cannot read to the end (an unknown program or form, a script whose content it does
//! not read) goes to the reviewer in every step (ORC-339, the person's decision of 2026-10-29): parsing that
//! stopped is no reason to ask the person. The reviewer is told which parts and why ([`unread_parts`]), with the
//! step's permissions, and judges itself whether such a part may write in a read-only step, reach the network,
//! write outside the worktree, read a secret, install, or destroy; it asks the person when it may. What
//! Orchester did read of such a part still holds it: where its output goes, a credential or share its words
//! name, a write outside, and every other part of the line, each checked on its own. Only a request with no
//! command Orchester could read at all ([`ReviewHold::Unreadable`]) goes to the person unreviewed.
//! A Codex escalation (a command outside its sandbox)
//! always goes to the person. Orchester's `run_check` and `start_qa_app` (a project program outside every
//! sandbox) and a request too long to be reviewed whole go to the person too; a project rule a person made
//! for `start_qa_app` answers before auto mode looks ([`crate::run::ProjectRule`]).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{
  input::{UnreadPart, UnreadWhy},
  push::{PushFacts, disguised_git, is_push, reviewable_push},
};
use crate::{
  orchester_tools::auto_approvable,
  permission::{
    AgentPermissions, NetworkAccess, RepositoryAccess,
    category::{CommandCategory, CommandPart},
    known_safe::program_name,
  },
  run::answer::ApprovalSubject,
};

/// A hard rule that sends a request to the person without a review.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewHold {
  /// It reads a credential location or store.
  Secrets,
  /// Orchester found no command in it to read. Before ORC-339 (2026-10-29) also a part it could not read to the
  /// end, in a step whose network is off or that only reads; such a part now goes to the reviewer.
  Unreadable,
  /// It rewrites or discards history beyond the run's own branch, changes git beyond the run's worktree (another
  /// branch, a tag, the shared settings, the worktrees), or deletes outside the worktree.
  Destructive,
  /// It pushes something other than the run's own branch to `origin`, or forces a push.
  Push,
  /// The step only reads the repository, and the command may write.
  ReadOnlyStep,
  /// The step's network is off, and the command may reach it or installs.
  NetworkOff,
  /// It writes outside the worktree and the step's writable folders.
  OutsideFolders,
  /// The step's permissions were not recorded, so nothing tells what is within them.
  PermissionsUnknown,
  /// The request is longer than a review reads whole.
  TooLong,
  /// Codex asks to run it outside its sandbox.
  Escalation,
  /// It runs one of the project's programs, outside every sandbox.
  ProjectProgram,
}

/// What the hard rules know besides the request: the step's recorded permissions, where a push of the
/// run would go, whether the request is a Codex escalation, and which build output folders the worktree's
/// repository ignores.
#[derive(Clone, Copy, Debug, Default)]
pub struct HoldRules<'a> {
  pub permissions: Option<&'a AgentPermissions>,
  pub push: PushFacts<'a>,
  pub escalation: bool,
  /// The target directories of the request's Cargo builds in the worktree
  /// ([`crate::permission::category::BuildOutput::folder`]) that its repository ignores and tracks no file in, as
  /// git told; none known holds every build in the worktree in a read-only step.
  pub build_output: &'a [PathBuf],
}

/// The hard rule that sends `subject` to the person without a review, under `rules`; `None` when the
/// reviewer may judge it.
pub fn review_hold(subject: &ApprovalSubject, rules: &HoldRules<'_>) -> Option<ReviewHold> {
  let ApprovalSubject::Command(command) = subject else {
    return tool_hold(subject, rules.permissions);
  };
  if command.reads_secrets() {
    return Some(ReviewHold::Secrets);
  }
  // Approved, an escalation runs the command outside Codex's sandbox, beyond any step's permissions.
  if rules.escalation {
    return Some(ReviewHold::Escalation);
  }
  if disguised_git(command) {
    return Some(ReviewHold::Push);
  }
  let Some(permissions) = rules.permissions else {
    return Some(ReviewHold::PermissionsUnknown);
  };
  let parts = command.parts();
  if parts.is_empty() {
    return Some(ReviewHold::Unreadable);
  }
  if (0..parts.len()).any(|at| is_push(&parts[at]) && !reviewable_push(parts, at, &rules.push)) {
    return Some(ReviewHold::Push);
  }
  if parts
    .iter()
    .any(|part| part.category() == CommandCategory::HistoryDestructive && !is_push(part))
  {
    return Some(ReviewHold::Destructive);
  }
  outside_permissions(parts, permissions, rules.build_output)
}

/// The hard rule a part outside the step's permissions breaks: in a read-only step a part that may write
/// ([`may_write`]); the network or an install while it is off; a write outside the folders.
fn outside_permissions(
  parts: &[CommandPart],
  permissions: &AgentPermissions,
  ignored: &[PathBuf],
) -> Option<ReviewHold> {
  if permissions.repository() == RepositoryAccess::ReadOnly && parts.iter().any(|part| may_write(part, ignored)) {
    return Some(ReviewHold::ReadOnlyStep);
  }
  if permissions.network() == NetworkAccess::Off
    && parts
      .iter()
      .any(|part| part.category() == CommandCategory::Network || part.installs() || is_push(part))
  {
    return Some(ReviewHold::NetworkOff);
  }
  parts
    .iter()
    .any(|part| part.category() == CommandCategory::OutsideWrite || part.writes_outside())
    .then_some(ReviewHold::OutsideFolders)
}

/// Whether `part` may write in a read-only step as far as Orchester can tell: a part it read that writes anything
/// but a Cargo build's output in `ignored` ([`writes_build_output`]); a part it did not read to the end only when
/// what it did read writes (a redirect into a file, a write outside, an install or a tool's cache) or it runs Cargo
/// in a form Orchester does not know, which builds and writes as the person's decision of 2026-10-18 holds. What it
/// did not read the reviewer judges, told the step only reads.
fn may_write(part: &CommandPart, ignored: &[PathBuf]) -> bool {
  if unread(part) {
    return part.writes_output()
      || part.writes_outside()
      || part.installs()
      || part.updates_cache()
      || part
        .words()
        .iter()
        .any(|word| program_name(word).as_deref() == Some("cargo"));
  }
  !part.writes_nothing() && !writes_build_output(part, ignored)
}

/// Whether `part` writes only a Cargo build's output: into a writable folder outside the worktree, or into a folder
/// of the worktree that git told is `ignored`.
fn writes_build_output(part: &CommandPart, ignored: &[PathBuf]) -> bool {
  part
    .build_output()
    .is_some_and(|output| output.in_worktree().is_none() || ignored.iter().any(|folder| folder == output.folder()))
}

/// Whether Orchester could not read `part` to the end: an unknown program or form, or a script.
fn unread(part: &CommandPart) -> bool {
  part.category() == CommandCategory::Unknown || part.runs_script()
}

/// The parts of `subject` Orchester could not read to the end, and why; the reviewer is told.
pub fn unread_parts(subject: &ApprovalSubject) -> Vec<UnreadPart> {
  let ApprovalSubject::Command(command) = subject else {
    return Vec::new();
  };
  command
    .parts()
    .iter()
    .enumerate()
    .filter(|(_, part)| unread(part))
    .map(|(at, part)| UnreadPart {
      at: at + 1,
      why: if part.runs_script() {
        UnreadWhy::Script
      } else {
        UnreadWhy::Unknown
      },
    })
    .collect()
}

/// The hard rule for a call of an MCP tool: Orchester's `run_check` never goes to the reviewer.
fn tool_hold(subject: &ApprovalSubject, permissions: Option<&AgentPermissions>) -> Option<ReviewHold> {
  match subject {
    ApprovalSubject::McpTool { server, tool, .. } if !auto_approvable(server, tool) => Some(ReviewHold::ProjectProgram),
    _ => permissions.is_none().then_some(ReviewHold::PermissionsUnknown),
  }
}
