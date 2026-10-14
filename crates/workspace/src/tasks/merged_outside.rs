//! Runs whose work a person merged without Orchester, with git or through a pull request that came back into
//! the checkout: a task's latest run that succeeded reads as merged once the branch it started from holds its
//! branch's tip. Git tells it once per reading of the tasks, in one listing for each such branch.

use std::{
  collections::{HashMap, HashSet},
  path::Path,
};

use orchester_core::{
  id::TaskId,
  integration::merged_into,
  run::{BranchName, Run, TaskPhase},
};
use orchester_exec::programs::Programs;

use super::{
  changes::GIT,
  list::{Latest, Phases},
};

/// A latest run that succeeded and was not merged in Orchester: its task, its branch and the branch merging
/// it brings its work into.
struct Succeeded {
  task: TaskId,
  branch: BranchName,
  into: BranchName,
}

/// The latest runs a reading of the tasks found succeeded and not merged.
#[derive(Default)]
pub(super) struct Unmerged(Vec<Succeeded>);

impl Unmerged {
  /// Adds `run`, the latest run of `task`, which succeeded and was not merged; a run that recorded no branch
  /// to merge into has none to look at.
  pub(super) fn add(&mut self, task: TaskId, run: &Run) {
    if let Some(into) = run.base_branch() {
      self.0.push(Succeeded {
        task,
        branch: run.branch().clone(),
        into: into.clone(),
      });
    }
  }

  /// Marks each run of `phases` whose branch's tip its base branch in the checkout at `repo_root` holds as
  /// merged into it.
  pub(super) fn settle(self, programs: &Programs, repo_root: &Path, phases: &mut Phases) {
    let mut held: HashMap<BranchName, HashSet<BranchName>> = HashMap::new();
    for run in self.0 {
      let merged = held
        .entry(run.into.clone())
        .or_insert_with(|| merged_branches(programs, repo_root, &run.into));
      if merged.contains(&run.branch)
        && let Some(Latest::Run(latest)) = phases.get_mut(&run.task)
      {
        latest.phase = TaskPhase::Merged;
        latest.merged_into = Some(run.into.as_str().into());
      }
    }
  }
}

/// The branches `into` holds; none when git cannot tell, so its runs keep reading as ready to merge, which a
/// merge in Orchester settles. The log says why.
fn merged_branches(programs: &Programs, repo_root: &Path, into: &BranchName) -> HashSet<BranchName> {
  merged_into(&programs.runner(), Path::new(GIT), repo_root, into).unwrap_or_else(|error| {
    tracing::warn!(%error, branch = %into, "cannot tell which runs were merged outside Orchester");
    HashSet::new()
  })
}
