//! What git told of runs' work, kept for this session: the task list and the task's page read the tasks again on
//! every change of a run, and asking git about every run that passed every step each time cost seconds. What git
//! says of a run depends only on the tips of its branch and of its base branch, so an answer stands until one of
//! them moves; one git found merged stays merged.

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::LazyLock,
};

use orchester_core::integration::RunWork;
use parking_lot::Mutex;

use super::Delivery;

/// The most answers kept; past it they are dropped all at once, and the next reading asks git again.
const LIMIT: usize = 4096;

static KNOWN: LazyLock<Mutex<HashMap<Asked, Told>>> = LazyLock::new(Default::default);

/// A run's work, in the checkout git was asked about it in.
#[derive(PartialEq, Eq, Hash)]
struct Asked {
  repo_root: PathBuf,
  work: RunWork,
}

impl Asked {
  fn new(repo_root: &Path, work: &RunWork) -> Self {
    Self {
      repo_root: repo_root.to_path_buf(),
      work: work.clone(),
    }
  }
}

/// What git said of a run's work, and the tips it said it of.
struct Told {
  tips: Tips,
  delivery: Delivery,
}

/// The commits a run's branch and its base branch pointed at when git was asked; `None` for one that was not a
/// branch then.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Tips {
  pub(super) branch: Option<String>,
  pub(super) into: Option<String>,
}

/// What git said of `work` in the checkout at `repo_root`, when it found it merged: that stays so whatever its
/// branches do after, and no git need run for it.
pub(super) fn merged(repo_root: &Path, work: &RunWork) -> Option<Delivery> {
  KNOWN
    .lock()
    .get(&Asked::new(repo_root, work))
    .map(|told| &told.delivery)
    .filter(|delivery| matches!(delivery, Delivery::MergedOutside(_)))
    .cloned()
}

/// What git said of `work` in the checkout at `repo_root` while its branches stood at `tips`.
pub(super) fn told(repo_root: &Path, work: &RunWork, tips: &Tips) -> Option<Delivery> {
  KNOWN
    .lock()
    .get(&Asked::new(repo_root, work))
    .filter(|told| told.tips == *tips)
    .map(|told| told.delivery.clone())
}

/// Keeps `delivery`, what git said of `work` in the checkout at `repo_root` with its branches at `tips`.
pub(super) fn keep(repo_root: &Path, work: &RunWork, tips: Tips, delivery: Delivery) {
  let mut known = KNOWN.lock();
  if known.len() >= LIMIT {
    known.clear();
  }
  known.insert(Asked::new(repo_root, work), Told { tips, delivery });
}
