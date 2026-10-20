//! How story points tracked reality: once a task is merged, its actual effort (tokens, active agent time,
//! attempts) is kept beside the points it had then ([`CalibrationRecord`]). [`calibration`] reads, per points
//! value, the median effort of the merged tasks that had it; [`comparables`] picks the merged tasks most like a new
//! one, which the estimator reads as examples.

use std::collections::{BTreeMap, HashSet};

use super::{EstimateSource, StoryPoints};
use crate::{id::TaskId, task::TaskDetails, time::Timestamp};

/// The most comparable tasks the estimator reads.
pub const COMPARABLES: usize = 8;

/// Words shorter than this tell nothing about what a task is about.
const SHORTEST_WORD: usize = 4;

/// What a merged task took, over all its runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TaskEffort {
  /// Tokens its agents and auto mode's reviewer spent, as budgets count them.
  pub tokens: u64,
  /// Active agent time, waits for a person left out.
  pub active_ms: u64,
  /// Attempts over all its runs: a retry or a return of its work is one more.
  pub attempts: u32,
  pub runs: u32,
}

/// A merged task's points, as it had them when it was merged, beside what it took.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalibrationRecord {
  pub task: TaskId,
  /// `ORC-12`.
  pub key: String,
  pub title: String,
  pub brief: String,
  /// `None`: it had no points when it was merged.
  pub points: Option<(StoryPoints, EstimateSource)>,
  pub effort: TaskEffort,
  pub merged_at: Timestamp,
}

/// The merged tasks of one points value and their median effort.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointsCalibration {
  pub points: StoryPoints,
  pub tasks: usize,
  pub median_tokens: u64,
  pub median_active_ms: u64,
  pub median_attempts: u32,
}

/// Per points value that merged tasks had, smallest first, the median effort of those tasks. Of an even number of
/// tasks the median is the lower middle one, a value one of them took.
pub fn calibration(records: &[CalibrationRecord]) -> Vec<PointsCalibration> {
  let mut by_points: BTreeMap<StoryPoints, Vec<TaskEffort>> = BTreeMap::new();
  for record in records {
    if let Some((points, _)) = record.points {
      by_points.entry(points).or_default().push(record.effort);
    }
  }
  by_points
    .into_iter()
    .map(|(points, efforts)| PointsCalibration {
      points,
      tasks: efforts.len(),
      median_tokens: median(efforts.iter().map(|effort| effort.tokens)),
      median_active_ms: median(efforts.iter().map(|effort| effort.active_ms)),
      median_attempts: median(efforts.iter().map(|effort| effort.attempts)),
    })
    .collect()
}

/// The lower middle of `values`, which are not empty.
fn median<T: Ord + Copy>(values: impl Iterator<Item = T>) -> T {
  let mut values: Vec<T> = values.collect();
  values.sort_unstable();
  values[(values.len() - 1) / 2]
}

/// The merged tasks with points most like `details`, at most [`COMPARABLES`]: those sharing the most words of
/// their title and brief, the latest merged first among equals.
pub fn comparables<'a>(records: &'a [CalibrationRecord], details: &TaskDetails) -> Vec<&'a CalibrationRecord> {
  let wanted = words(&format!("{} {}", details.title.as_str(), details.brief));
  let mut scored: Vec<(usize, &CalibrationRecord)> = records
    .iter()
    .filter(|record| record.points.is_some())
    .map(|record| {
      let shared = words(&format!("{} {}", record.title, record.brief))
        .intersection(&wanted)
        .count();
      (shared, record)
    })
    .collect();
  scored.sort_by(|(left_shared, left), (right_shared, right)| {
    right_shared
      .cmp(left_shared)
      .then_with(|| right.merged_at.cmp(&left.merged_at))
  });
  scored
    .into_iter()
    .take(COMPARABLES)
    .map(|(_, record)| record)
    .collect()
}

/// The words of `text` that tell what it is about, lower case.
fn words(text: &str) -> HashSet<String> {
  text
    .split(|c: char| !c.is_alphanumeric())
    .filter(|word| word.chars().count() >= SHORTEST_WORD)
    .map(str::to_lowercase)
    .collect()
}
