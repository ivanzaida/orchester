//! What the closing panel lists while a close is held, live from the runners' reports and the projects'
//! git operations (`Row states` in `mKfVy`): while the person decides, every run with an agent at work;
//! while the agents save, every run the runners report, saved ones included, so a run that starts or
//! settles meanwhile shows as it is. The window closes only once the runners stopped and git is done;
//! until then Quit now stays offered.

use std::sync::Arc;

use super::ClosePhase;
use crate::{
  runner::{ActiveWork, Runners, SaveState},
  tasks::integration_jobs::{BusyJob, ProjectJobs},
};

/// Where a run stands on the panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunStands {
  /// Its agent works; the person decides.
  Running,
  /// Its agent finishes up to its next checkpoint.
  Saving,
  /// Settled: paused at this checkpoint (a full revision), or without a new one.
  Saved { checkpoint: Option<Arc<str>> },
}

/// What the panel says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
  /// The person decides (`u3Fw36`).
  Confirm,
  /// The agents save, or git finishes (`lxm8W`).
  Waiting,
  /// Every run is saved and git is done; the runners exit and Orchester closes (`n5psM`).
  Saved,
}

/// The runs and the git operations the panel lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
  pub runs: Vec<(ActiveWork, RunStands)>,
  pub jobs: Vec<BusyJob>,
  waiting: bool,
}

impl Board {
  /// The board of a close in `phase`, from what `runners` last reported and what `jobs` do now.
  pub fn of(phase: &ClosePhase, runners: &Runners, jobs: &ProjectJobs) -> Self {
    let waiting = matches!(phase, ClosePhase::Waiting { .. });
    let runs = if waiting {
      runners
        .board()
        .into_iter()
        .map(|(work, save)| (work, stands(save)))
        .collect()
    } else {
      runners
        .active_work()
        .into_iter()
        .map(|work| (work, RunStands::Running))
        .collect()
    };
    Self::new(runs, jobs.busy(), waiting)
  }

  /// The board of `runs` and `jobs`, while the agents save when `waiting`.
  pub fn new(runs: Vec<(ActiveWork, RunStands)>, jobs: Vec<BusyJob>, waiting: bool) -> Self {
    Self { runs, jobs, waiting }
  }

  pub fn stage(&self) -> Stage {
    if !self.waiting {
      Stage::Confirm
    } else if self.left() == 0 {
      Stage::Saved
    } else {
      Stage::Waiting
    }
  }

  /// How many runs still save, and git operations still run.
  pub fn left(&self) -> usize {
    let saving = self
      .runs
      .iter()
      .filter(|(_, stands)| !matches!(stands, RunStands::Saved { .. }))
      .count();
    saving + self.jobs.len()
  }
}

fn stands(save: SaveState) -> RunStands {
  match save {
    SaveState::Saving => RunStands::Saving,
    SaveState::Saved { checkpoint } => RunStands::Saved { checkpoint },
  }
}
