//! What the Orchestrator chat did, or proposes, with git in the project's repository (`docs/design/
//! orchestrator-toolset.md`, decision 9): a write it ran itself under the person's switch *Let the Orchestrator write
//! with git* (a commit, a branch, a switch, a pull, a push, a merge, a revert, a tag, a fetch), kept with the commands
//! and what git said so the person sees it, or an operation that can lose work, which is only ever proposed and which
//! only a person applies, whatever the switch says. The closed set of [`GitProposal`]s is every such operation the chat
//! has: what it is not offered it cannot do.

use serde::{Deserialize, Serialize};

use crate::id::RunId;

/// A git action of the chat's: where it ran, what it is, the commands it ran or would run as people read them, and for
/// a proposal what was read when it was proposed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitChange {
  pub place: GitPlace,
  pub operation: GitOperation,
  /// The git commands, as people read them (`git commit -F - -- src/a.rs`).
  pub commands: Vec<String>,
  /// What git said of a write it ran, its last lines.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub said: Option<String>,
  /// A proposal's basis: the commits its refs pointed at when it was proposed (`HEAD=…`, `origin/main=…`); applying it
  /// is refused once they moved.
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub basis: String,
  /// What a proposal would lose, as read when it was proposed: commits, changes, files, branches or tags, one per
  /// line.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub loses: Vec<String>,
}

/// Where git runs: the project's checkout, or the worktree of run `run`, `#number`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GitPlace {
  Checkout,
  Run { run: RunId, number: u32 },
}

/// A write the chat ran itself, or an operation that can lose work it proposed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitOperation {
  Write(GitWrite),
  Propose(GitProposal),
}

impl GitOperation {
  /// The git command as people read it, for a card's title (`git reset --hard`).
  pub const fn command(&self) -> &'static str {
    match self {
      Self::Write(write) => write.command(),
      Self::Propose(proposal) => proposal.command(),
    }
  }
}

/// A write the chat may run itself while the person's switch is on: none of them loses work, and each that would is
/// refused by git or stopped and undone before anything is left half-done.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitWrite {
  Commit,
  CreateBranch,
  Switch,
  Pull,
  Push,
  Merge,
  Revert,
  Tag,
  Fetch,
}

impl GitWrite {
  pub const fn command(self) -> &'static str {
    match self {
      Self::Commit => "git commit",
      Self::CreateBranch => "git branch",
      Self::Switch => "git switch",
      Self::Pull => "git pull",
      Self::Push => "git push",
      Self::Merge => "git merge",
      Self::Revert => "git revert",
      Self::Tag => "git tag",
      Self::Fetch => "git fetch",
    }
  }
}

/// An operation that can lose work: only ever proposed, and applied by a person.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GitProposal {
  /// Move the current branch to `to`: its commits after `to` leave it, and `hard` discards uncommitted changes too.
  Reset { to: String, mode: ResetMode },
  /// Delete untracked files (in `paths`, or everywhere), with untracked `directories`, and `ignored` files too.
  Clean {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    paths: Vec<String>,
    directories: bool,
    ignored: bool,
  },
  /// Delete `branch`, here or, with `remote`, on that remote: its commits no other branch holds are lost.
  DeleteBranch {
    branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remote: Option<String>,
  },
  /// Push `branch` to `remote` over what the remote holds (`--force-with-lease`, over only what was proposed).
  ForcePush { remote: String, branch: String },
  /// Discard the uncommitted changes to the tracked files `paths`, staged and not.
  DiscardChanges { paths: Vec<String> },
  /// Switch to `branch`, discarding the uncommitted changes to tracked files.
  SwitchDiscarding { branch: String },
  /// Rebase the current branch onto `onto`: its commits are written anew.
  Rebase { onto: String },
  /// Delete tag `tag`, here or, with `remote`, on that remote.
  DeleteTag {
    tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remote: Option<String>,
  },
  /// Abort the merge in progress, discarding what it changed and every resolution made in it.
  AbortMerge,
}

impl GitProposal {
  pub const fn command(&self) -> &'static str {
    match self {
      Self::Reset {
        mode: ResetMode::Hard, ..
      } => "git reset --hard",
      Self::Reset {
        mode: ResetMode::Mixed, ..
      } => "git reset --mixed",
      Self::Reset {
        mode: ResetMode::Soft, ..
      } => "git reset --soft",
      Self::Clean { .. } => "git clean",
      Self::DeleteBranch { remote: None, .. } => "git branch -D",
      Self::DeleteBranch { remote: Some(_), .. } | Self::DeleteTag { remote: Some(_), .. } => "git push --delete",
      Self::ForcePush { .. } => "git push --force-with-lease",
      Self::DiscardChanges { .. } => "git restore",
      Self::SwitchDiscarding { .. } => "git switch --discard-changes",
      Self::Rebase { .. } => "git rebase",
      Self::DeleteTag { remote: None, .. } => "git tag -d",
      Self::AbortMerge => "git merge --abort",
    }
  }
}

/// How a reset moves the branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResetMode {
  /// The branch, the index and the tracked files.
  Hard,
  /// The branch and the index; the files keep their changes.
  Mixed,
  /// The branch only.
  Soft,
}
