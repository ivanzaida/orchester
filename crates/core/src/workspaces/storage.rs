//! What could be cleaned to make room while the disk guard holds (ORC-422): the folders Orchester keeps that no run
//! works in. The worktrees of ended runs go first — removing one frees all of it and keeps its branch — then the
//! idle warm workspaces, whose warm folders cleanup deletes ([`super::cleanup`]); each group the largest first.

use std::{cmp::Reverse, path::PathBuf};

use super::WorkspaceNumber;
use crate::id::RunId;

/// A folder Orchester keeps that could be cleaned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cleanable {
  pub kind: CleanableKind,
  pub path: PathBuf,
  /// What it holds on disk; `None` when it could not be read.
  pub bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanableKind {
  /// The worktree of a run that ended.
  EndedRunWorktree { run: RunId },
  /// A warm workspace nobody holds and no run left changes in.
  IdleWorkspace { number: WorkspaceNumber },
}

/// `cleanable` in the order to clean: ended runs' worktrees first, then idle workspaces, each the largest first and
/// those whose size could not be read last.
pub fn cleanable_order(mut cleanable: Vec<Cleanable>) -> Vec<Cleanable> {
  cleanable.sort_by_key(|item| {
    let workspace = matches!(item.kind, CleanableKind::IdleWorkspace { .. });
    (workspace, Reverse(item.bytes))
  });
  cleanable
}
