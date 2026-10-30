//! The disk guard: free space to keep on the disk the workspaces are on, which is the repository's disk since
//! `.orchester/` lives inside it. Below it no new step starts, and working steps pause with a checkpoint and go on
//! once there is room again; nothing fails for space. How much is free is read through [`DiskSpace`], a boundary
//! like the clock: the runner and the app read the real disk, tests say what it holds.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// A gigabyte as the person reads it on Windows and macOS alike: 2^30 bytes.
pub const GIGABYTE: u64 = 1 << 30;

/// How much space a disk has, in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiskUsage {
  /// What this process may still write.
  pub free: u64,
  pub total: u64,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DiskSpaceError {
  #[error("cannot read the free space of the disk {path} is on: {detail}")]
  Unreadable { path: PathBuf, detail: String },
}

/// Reads how much space the disk of a folder has.
pub trait DiskSpace: Send + Sync {
  fn usage(&self, folder: &Path) -> Result<DiskUsage, DiskSpaceError>;
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("keep free must be from {min} to {max} GB", min = KeepFree::MIN_GB, max = KeepFree::MAX_GB)]
pub struct KeepFreeError {
  pub gigabytes: u64,
}

/// Free space the disk guard keeps, in whole gigabytes: a setting of the project on its own (Project settings › Runs ›
/// Storage), which the guard holds for every project, whether its runs borrow warm workspaces or own a worktree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeepFree(u64);

impl KeepFree {
  pub const MIN_GB: u64 = 1;
  /// Far beyond any disk a computer has; a bound only so bytes never overflow.
  pub const MAX_GB: u64 = 1 << 20;
  /// 50 GB, for a project whose person never set it: room for a few builds before the disk fills.
  pub const DEFAULT: Self = Self(50);

  pub const fn from_gigabytes(gigabytes: u64) -> Result<Self, KeepFreeError> {
    if gigabytes < Self::MIN_GB || gigabytes > Self::MAX_GB {
      return Err(KeepFreeError { gigabytes });
    }
    Ok(Self(gigabytes))
  }

  pub const fn gigabytes(self) -> u64 {
    self.0
  }

  pub const fn bytes(self) -> u64 {
    self.0 * GIGABYTE
  }

  /// Whether `usage` leaves room for new work.
  pub const fn verdict(self, usage: DiskUsage) -> DiskVerdict {
    if usage.free >= self.bytes() {
      DiskVerdict::Room
    } else {
      DiskVerdict::Low {
        free: usage.free,
        needed: self.bytes(),
      }
    }
  }
}

impl Default for KeepFree {
  fn default() -> Self {
    Self::DEFAULT
  }
}

/// Whether the disk has room for new work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskVerdict {
  Room,
  /// `free` bytes are below the `needed` the person keeps free: new steps wait, working ones pause.
  Low {
    free: u64,
    needed: u64,
  },
}

impl DiskVerdict {
  pub const fn is_low(self) -> bool {
    matches!(self, Self::Low { .. })
  }
}
