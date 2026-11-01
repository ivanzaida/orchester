//! Warm workspaces (ORC-114): a project's few reusable folders, `.orchester/workspaces/1..N`, that runs borrow
//! instead of owning a worktree each. N is the project's "Agents working at once" ([`crate::limits::AgentSlots`]):
//! each agent at work uses one workspace, and so does the merge queue's re-check, which builds.
//!
//! - A run is its branch plus checkpoint commits. When a step starts, it takes a free workspace — the one it used last,
//!   else the one idle longest ([`pool`]) — and waits with the reason when none is free. Git switches the workspace to
//!   the run's branch and cleans every ignored file but the warm folders kept between runs ([`warm`]). The run keeps it
//!   while its steps follow one another and gives it back, after a checkpoint commit, when it waits or ends.
//! - Orchester knows no languages: it suggests the ignored folders of the person's checkout that are worth keeping only
//!   by their size ([`suggest`]).
//! - What runs write for people to see lives in the run's own storage, `.orchester/runs/<run-id>/`, which cleaning or
//!   reusing a workspace never touches ([`artifacts_folder`]).
//! - The disk guard keeps free space on the repository's disk ([`disk`]) in every project, workspaces on or off: below
//!   it no step starts and working steps pause with a checkpoint; cleanup deletes warm folders of the workspaces idle
//!   longest first ([`cleanup`]); while it holds, the project lists what could be cleaned ([`storage`]).
//!
//! The rules live here; the runner and the app do git and the filesystem.

pub mod cleanup;
pub mod disk;
pub mod pool;
pub mod settings;
pub mod storage;
mod store;
pub mod suggest;
pub mod warm;

use std::{
  fmt,
  num::NonZeroU32,
  path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub use self::{
  cleanup::{CleanupChoice, cleanup_order, default_cleanup},
  disk::{DiskSpace, DiskSpaceError, DiskUsage, DiskVerdict, GIGABYTE, KeepFree, KeepFreeError},
  pool::{Borrow, Holder, Stranded, WaitReason, Warmth, WorkspaceRecord, WorkspaceState, choose},
  settings::{WarmUpCommand, WarmUpCommandError, WarmUpFailure, WorkspaceSettings},
  storage::{Cleanable, CleanableKind, cleanable_order},
  store::WorkspaceStore,
  suggest::{IgnoredFolder, SUGGEST_DEPTH, SUGGEST_OVER, Suggestion, suggest},
  warm::{Cleaning, WarmChoice, WarmFolder, WarmPattern, WarmPatternError, cleaning},
};
use crate::{id::RunId, run::BranchName};

/// Where a project's workspaces are, relative to its repository.
pub const WORKSPACES_DIR: &str = ".orchester/workspaces";
/// Where runs keep what they write for people to see, relative to the repository: `<dir>/<run-id>/`.
pub const ARTIFACTS_DIR: &str = ".orchester/runs";

/// The number of one of a project's workspaces, from 1: its folder is `.orchester/workspaces/<number>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceNumber(NonZeroU32);

impl WorkspaceNumber {
  pub const FIRST: Self = Self(NonZeroU32::MIN);

  /// `number` when it is above zero.
  pub const fn new(number: u32) -> Option<Self> {
    match NonZeroU32::new(number) {
      Some(number) => Some(Self(number)),
      None => None,
    }
  }

  pub const fn get(self) -> u32 {
    self.0.get()
  }

  /// The workspace's folder in the repository at `repo`.
  pub fn path(self, repo: &Path) -> PathBuf {
    repo.join(WORKSPACES_DIR).join(self.to_string())
  }

  /// The workspace's folder as git reads it in the repository's folder.
  pub fn relative(self) -> String {
    format!("{WORKSPACES_DIR}/{self}")
  }

  /// The workspaces a project with `limit` agents at once may use, `1..=limit`.
  pub fn up_to(limit: NonZeroU32) -> impl Iterator<Item = Self> {
    (1..=limit.get()).filter_map(Self::new)
  }
}

impl fmt::Display for WorkspaceNumber {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

/// The run's own storage in the repository at `repo`: `.orchester/runs/<run-id>/`. Its agents read and write there;
/// the task page lists it under Artifacts.
pub fn artifacts_folder(repo: &Path, run: RunId) -> PathBuf {
  repo.join(ARTIFACTS_DIR).join(run.to_string())
}

/// The branch that keeps the uncommitted changes a run left in a workspace it could not give back
/// ("Keep them in a branch"): the run's branch `orchester/kon-18-run-4` gives `orchester/rescue/kon-18-run-4`. A
/// branch of another name, one a person gave, gets the prefix in front.
pub fn rescue_branch(run_branch: &BranchName) -> BranchName {
  let name = run_branch.as_str();
  let rest = name.strip_prefix("orchester/").unwrap_or(name);
  BranchName::new(&format!("orchester/rescue/{rest}")).expect("a valid branch name behind a valid prefix stays valid")
}
