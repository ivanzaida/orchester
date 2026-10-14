//! What became of the work of a run that passed every step, as git tells it beyond a merge Orchester recorded:
//! a run that committed nothing has nothing to merge, and one whose commits its base branch already holds was
//! merged by a person without Orchester, with git or through a pull request pulled back into the checkout.
//! What git cannot see stays waiting to be merged: a squash-merged or rebased pull request (the base holds
//! other commits), a base branch not pulled since, and a run branch deleted or force-pushed.

use std::{
  collections::{HashMap, HashSet},
  path::Path,
};

use super::{GitError, git::Git};
use crate::{
  command::CommandRunner,
  revision::Revision,
  run::{BranchName, Run},
};

const HEADS: &str = "refs/heads/";

/// Where the work of a run that passed every step stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivered {
  /// Its branch has commits past the revision the run started from that its base branch does not all hold,
  /// or git cannot find its branch or its base branch: it waits to be merged.
  Waiting,
  /// Its branch has no commit past the revision the run started from: there is nothing to merge.
  NothingToMerge,
  /// Its branch has commits past the revision the run started from, and its base branch holds them all.
  Merged,
}

/// What a run's work is in git: its branch, the revision it started from, and the branch merging it brings it
/// into, when the run recorded one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunWork {
  pub branch: BranchName,
  pub base: Revision,
  pub into: Option<BranchName>,
}

impl RunWork {
  pub fn of(run: &Run) -> Self {
    Self {
      branch: run.branch().clone(),
      base: run.base().clone(),
      into: run.base_branch().cloned(),
    }
  }
}

/// The local branches of a repository, read to tell what became of runs' work: every branch's tip in one
/// `git for-each-ref`, and which branches a base branch holds in one more for each base branch asked about.
pub struct Branches<'a, R: CommandRunner> {
  git: Git<'a, R>,
  /// Each branch's tip, by name.
  tips: HashMap<String, String>,
  /// The branches each base branch asked about holds, by name.
  held: HashMap<BranchName, HashSet<String>>,
}

impl<'a, R: CommandRunner> Branches<'a, R> {
  /// The branches of the repository at `repo`, read with the `git` program.
  pub fn read(runner: &'a R, git: &'a Path, repo: &'a Path) -> Result<Self, GitError> {
    let git = Git::new(runner, git, repo);
    let listed = git.stdout(&["for-each-ref", "--format=%(objectname) %(refname)", HEADS])?;
    let tips = listed
      .lines()
      .filter_map(|line| {
        let (tip, name) = line.trim().split_once(' ')?;
        Some((name.strip_prefix(HEADS)?.to_owned(), tip.to_owned()))
      })
      .collect();
    Ok(Self {
      git,
      tips,
      held: HashMap::new(),
    })
  }

  /// What became of `work`. Only a run git finds commits of in its base branch reads as merged; a branch
  /// whose tip is the revision its run started from, or one the run's start already holds, has nothing to
  /// merge. An error is about `work.into`, the one base branch git was asked about then.
  pub fn delivered(&mut self, work: &RunWork) -> Result<Delivered, GitError> {
    let Some(tip) = self.tips.get(work.branch.as_str()).cloned() else {
      return Ok(Delivered::Waiting);
    };
    if tip == work.base.as_str() {
      return Ok(Delivered::NothingToMerge);
    }
    let Some(into) = &work.into else {
      return Ok(Delivered::Waiting);
    };
    if !self.held(into)?.contains(work.branch.as_str()) {
      return Ok(Delivered::Waiting);
    }
    // Held, but maybe only because the branch went back to where the run started: then it has nothing of its own.
    if self.git.is_ancestor(&tip, work.base.as_str())? {
      return Ok(Delivered::NothingToMerge);
    }
    Ok(Delivered::Merged)
  }

  /// The branches `into` holds; none when it is not a branch here.
  fn held(&mut self, into: &BranchName) -> Result<&HashSet<String>, GitError> {
    if !self.held.contains_key(into) {
      let held = if self.tips.contains_key(into.as_str()) {
        let listed = self.git.stdout(&[
          "for-each-ref",
          "--format=%(refname)",
          &format!("--merged={HEADS}{into}"),
          HEADS,
        ])?;
        listed
          .lines()
          .filter_map(|line| line.trim().strip_prefix(HEADS))
          .map(str::to_owned)
          .collect()
      } else {
        HashSet::new()
      };
      self.held.insert(into.clone(), held);
    }
    Ok(&self.held[into])
  }
}
