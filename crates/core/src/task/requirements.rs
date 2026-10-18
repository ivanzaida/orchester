//! What a task asked for at each moment Orchester recorded it, for the task page's Details: as the task
//! was created, as its latest run was given it when it started, and as it is now. Both recorded versions
//! are immutable; a version that was not recorded is never made up from another, so a task created before
//! creation versions were kept says so and shows the version its run kept, or its current one.

use super::TaskDetails;
use crate::{id::RunId, run::RunNumber};

/// A version of a task's requirements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RequirementsSource {
  /// As the task was created.
  Creation,
  /// As the run was given them when it started.
  RunStart(RunId),
  /// The task as it is now, which every edit changes.
  Current,
}

/// A run and the requirements it started with, when they were recorded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunStart {
  pub run: RunId,
  pub number: RunNumber,
  pub details: Option<TaskDetails>,
}

/// The task's latest run, as far as it is known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LatestRun {
  /// The task never had a run.
  None,
  /// It had runs, but which one is the latest cannot be read.
  Unknown,
  /// Its runs ran on another computer: a shared task's holder reported this one, and nothing it started
  /// with is on this computer.
  Elsewhere(RunNumber),
  Run(RunStart),
}

impl LatestRun {
  /// Whether the task had a run, readable here or not.
  pub const fn ran(&self) -> bool {
    !matches!(self, Self::None)
  }
}

/// Why the version shown stands in for the creation version, which was not recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fallback {
  /// The latest run's start version is shown instead.
  RunStart,
  /// No earlier version was recorded: the current one is shown.
  Current,
}

/// How the current requirements stand against what the latest run started with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurrentStanding {
  NoRun,
  EditedAfter(RunNumber),
  UnchangedSince(RunNumber),
  /// What the run started with was not recorded, so whether the task changed since is not known.
  RunStartNotRecorded(RunNumber),
  /// The latest run cannot be read, so whether the task changed since it started is not known.
  RunUnknown,
  /// The latest run ran on another computer, so what it started with is not known here.
  RunElsewhere(RunNumber),
}

/// A task's recorded versions of its requirements and its current one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskRequirements {
  current: TaskDetails,
  creation: Option<TaskDetails>,
  latest: LatestRun,
}

impl TaskRequirements {
  pub const fn new(current: TaskDetails, creation: Option<TaskDetails>, latest: LatestRun) -> Self {
    Self {
      current,
      creation,
      latest,
    }
  }

  /// The versions that can be shown, oldest first.
  pub fn sources(&self) -> Vec<RequirementsSource> {
    let creation = self.creation.is_some().then_some(RequirementsSource::Creation);
    let run = self.recorded_run().map(|run| RequirementsSource::RunStart(run.run));
    creation
      .into_iter()
      .chain(run)
      .chain([RequirementsSource::Current])
      .collect()
  }

  /// The version shown until another is chosen: the oldest one recorded, else the current one.
  pub fn default_source(&self) -> RequirementsSource {
    if self.creation.is_some() {
      return RequirementsSource::Creation;
    }
    self
      .recorded_run()
      .map_or(RequirementsSource::Current, |run| RequirementsSource::RunStart(run.run))
  }

  /// The version shown for `chosen`: the chosen one while it can be shown, else the default.
  pub fn shown(&self, chosen: Option<RequirementsSource>) -> RequirementsSource {
    chosen
      .filter(|source| self.details(*source).is_some())
      .unwrap_or_else(|| self.default_source())
  }

  /// The requirements of `source`; `None` for a version that was not recorded.
  pub fn details(&self, source: RequirementsSource) -> Option<&TaskDetails> {
    match source {
      RequirementsSource::Creation => self.creation.as_ref(),
      RequirementsSource::RunStart(run) => self
        .recorded_run()
        .filter(|recorded| recorded.run == run)
        .and_then(|recorded| recorded.details.as_ref()),
      RequirementsSource::Current => Some(&self.current),
    }
  }

  /// The task's latest run, whether or not what it started with was recorded.
  pub const fn latest_run(&self) -> &LatestRun {
    &self.latest
  }

  /// Whether the version `shown` stands in for a creation version that was not recorded: only while it
  /// is the one shown by default.
  pub fn fallback(&self, shown: RequirementsSource) -> Option<Fallback> {
    if self.creation.is_some() || shown != self.default_source() {
      return None;
    }
    match shown {
      RequirementsSource::RunStart(_) => Some(Fallback::RunStart),
      RequirementsSource::Current => Some(Fallback::Current),
      RequirementsSource::Creation => None,
    }
  }

  pub fn current_standing(&self) -> CurrentStanding {
    let run = match &self.latest {
      LatestRun::None => return CurrentStanding::NoRun,
      LatestRun::Unknown => return CurrentStanding::RunUnknown,
      LatestRun::Elsewhere(number) => return CurrentStanding::RunElsewhere(*number),
      LatestRun::Run(run) => run,
    };
    match &run.details {
      None => CurrentStanding::RunStartNotRecorded(run.number),
      Some(started) if *started == self.current => CurrentStanding::UnchangedSince(run.number),
      Some(_) => CurrentStanding::EditedAfter(run.number),
    }
  }

  fn recorded_run(&self) -> Option<&RunStart> {
    match &self.latest {
      LatestRun::Run(run) if run.details.is_some() => Some(run),
      _ => None,
    }
  }
}
