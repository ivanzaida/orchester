//! What became of the work of a task's latest run that passed every step and that Orchester did not merge
//! (`orchester_core::integration::Branches`): a run that committed nothing has nothing to merge, and one whose
//! commits its base branch holds was merged by a person without Orchester, so it reads as merged into it. The
//! task list's rows and the task's page ask git the same way, once per reading, and only of runs whose branches
//! moved since git last told of them ([`known`]). What git cannot see waits to be merged until the task is Done:
//! a squash-merged or rebased pull request, a base branch not pulled, a run branch deleted or force-pushed.

mod known;

use std::{
  collections::HashSet,
  path::{Path, PathBuf},
  sync::{Arc, LazyLock},
};

use orchester_core::{
  command::CommandRunner,
  id::{RunId, TaskId},
  integration::{Branches, Delivered, GitError, RunWork},
  run::{BranchName, Run, TaskPhase},
};
use orchester_exec::programs::Programs;
use parking_lot::Mutex;

use super::{
  changes::GIT,
  history::RunRecord,
  list::{Latest, Phases},
};
use crate::messages::Message;

/// What became of a succeeded run's work that Orchester did not merge.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Delivery {
  /// It waits to be merged, as far as git tells.
  #[default]
  Waiting,
  /// The run committed nothing past the revision it started from: there is nothing to merge.
  NothingToMerge,
  /// A person merged its work into this branch without Orchester.
  MergedOutside(Arc<str>),
}

impl Delivery {
  /// The branch a person merged the work into without Orchester: the run's delivery target, its base branch,
  /// whichever branch the checkout has checked out. The Summary and the Local integration page both say merged
  /// without Orchester by this, and name this branch.
  pub fn merged_into(&self) -> Option<&str> {
    match self {
      Self::MergedOutside(into) => Some(into),
      Self::Waiting | Self::NothingToMerge => None,
    }
  }

  /// `phase`, of the run this tells of, as the task's page reads it: one that passed every step reads as merged once
  /// a person merged its work without Orchester.
  pub fn phase(&self, phase: TaskPhase) -> TaskPhase {
    match (phase, self) {
      (TaskPhase::Succeeded, Self::MergedOutside(_)) => TaskPhase::Merged,
      (phase, _) => phase,
    }
  }
}

/// What became of `run`'s work, a succeeded run that Orchester did not merge, in the checkout at `repo_root`.
pub fn of_run(programs: &Programs, repo_root: &Path, run: &Run) -> Delivery {
  deliveries(programs, repo_root, &[RunWork::of(run)]).remove(0)
}

/// Settles each of `records`, runs as *Runs of this task* lists them, that passed every step and that Orchester did
/// not merge, as git in the checkout at `repo_root` tells them, the way the Summary reads the latest: one whose work
/// its base branch holds reads as merged into it, all of them in one reading of the branches. Gives what became of
/// each of them, by run.
pub fn settle_records<'a>(
  programs: &Programs,
  repo_root: &Path,
  records: impl IntoIterator<Item = &'a mut RunRecord>,
) -> Vec<(RunId, Delivery)> {
  let (unmerged, works): (Vec<&mut RunRecord>, Vec<RunWork>) = records
    .into_iter()
    .filter(|record| record.phase == Some(TaskPhase::Succeeded))
    .filter_map(|record| {
      let work = RunWork::of(record.run.as_deref()?);
      Some((record, work))
    })
    .unzip();
  // One reading of the branches for all of them: one `git for-each-ref`, and one more for each base branch.
  unmerged
    .into_iter()
    .zip(deliveries(programs, repo_root, &works))
    .map(|(record, delivery)| {
      if let Delivery::MergedOutside(into) = &delivery {
        record.phase = Some(TaskPhase::Merged);
        record.detail = Message::new("runs.history.mergedOutside").with("branch", into.to_string());
      }
      (record.id, delivery)
    })
    .collect()
}

/// The latest runs a reading of the tasks found succeeded and not merged in Orchester, by task.
#[derive(Default)]
pub(super) struct Unmerged(Vec<(TaskId, RunWork)>);

impl Unmerged {
  /// Adds `run`, the latest run of `task`, which succeeded and was not merged in Orchester.
  pub(super) fn add(&mut self, task: TaskId, run: &Run) {
    self.0.push((task, RunWork::of(run)));
  }

  /// Settles each run of `phases` git tells more of in the checkout at `repo_root`: merged into its base branch,
  /// or with nothing to merge.
  pub(super) fn settle(self, programs: &Programs, repo_root: &Path, phases: &mut Phases) {
    let (tasks, works): (Vec<TaskId>, Vec<RunWork>) = self.0.into_iter().unzip();
    for (task, delivery) in tasks.into_iter().zip(deliveries(programs, repo_root, &works)) {
      let Some(Latest::Run(latest)) = phases.get_mut(&task) else {
        continue;
      };
      match delivery {
        Delivery::Waiting => {}
        Delivery::NothingToMerge => latest.nothing_to_merge = true,
        Delivery::MergedOutside(into) => {
          latest.phase = TaskPhase::Merged;
          latest.merged_into = Some(into);
        }
      }
    }
  }
}

/// What became of each of `works`, in their order: git lists the branches once for all of them, and is asked more
/// only of a run whose branch or base branch moved since this session last asked, or that it never asked of
/// ([`known`]). No git runs when git found every one of them merged already. What git cannot tell waits to be
/// merged, which a merge in Orchester settles.
pub(crate) fn deliveries(programs: &Programs, repo_root: &Path, works: &[RunWork]) -> Vec<Delivery> {
  let merged: Vec<Option<Delivery>> = works.iter().map(|work| known::merged(repo_root, work)).collect();
  if merged.iter().all(Option::is_some) {
    return merged.into_iter().flatten().collect();
  }
  let program = Path::new(GIT);
  let runner = programs.runner();
  let mut branches = match Branches::read(&runner, program, repo_root) {
    Ok(branches) => branches,
    Err(error) => {
      once(repo_root, None, &error);
      return merged.into_iter().map(Option::unwrap_or_default).collect();
    }
  };
  works
    .iter()
    .zip(merged)
    .map(|(work, merged)| merged.unwrap_or_else(|| delivered(&mut branches, repo_root, work)))
    .collect()
}

/// What became of `work`, as git told it last while its branches stood where they stand in `branches`, or as git
/// tells it now.
fn delivered<R: CommandRunner>(branches: &mut Branches<'_, R>, repo_root: &Path, work: &RunWork) -> Delivery {
  let tips = known::Tips {
    branch: branches.tip(&work.branch).map(str::to_owned),
    into: work
      .into
      .as_ref()
      .and_then(|into| branches.tip(into))
      .map(str::to_owned),
  };
  if let Some(told) = known::told(repo_root, work, &tips) {
    return told;
  }
  let delivery = match branches.delivered(work) {
    Ok(Delivered::Waiting) => Delivery::Waiting,
    Ok(Delivered::NothingToMerge) => Delivery::NothingToMerge,
    Ok(Delivered::Merged) => {
      let into = work
        .into
        .as_ref()
        .expect("only a run with a base branch reads as merged into it");
      Delivery::MergedOutside(into.as_str().into())
    }
    Err(error) => {
      // Not kept: git is asked again next time, and says it once (`once`).
      once(repo_root, work.into.as_ref(), &error);
      return Delivery::Waiting;
    }
  };
  known::keep(repo_root, work, tips, delivery.clone());
  delivery
}

/// What git could not tell, already logged in this session, by checkout and base branch: without git, every
/// reading of the tasks would say it again.
static LOGGED: LazyLock<Mutex<HashSet<Unanswered>>> = LazyLock::new(Default::default);

/// A checkout, and the base branch git was asked about there; `None` for its branches as a whole.
type Unanswered = (PathBuf, Option<BranchName>);

/// Logs `error`, about `into` in the checkout at `repo_root`, the first time this session meets it there.
fn once(repo_root: &Path, into: Option<&BranchName>, error: &GitError) {
  if LOGGED.lock().insert((repo_root.to_path_buf(), into.cloned())) {
    tracing::warn!(%error, repo_root = %repo_root.display(), branch = ?into.map(BranchName::as_str),
      "cannot tell what became of runs that passed every step; they read as waiting to be merged");
  }
}
