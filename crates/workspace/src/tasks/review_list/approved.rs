//! *Approved today*.

use std::collections::HashSet;

use orchester_core::{
  id::{RunId, TaskId},
  integration::{RunMerge, attempt::MergeAsker},
  merge_queue::QueueEntry,
  time::Timestamp,
};
use orchester_store::StoreError;

use super::{ReviewRow, ReviewState, rows::Reader};

/// How many merges are read at a time while looking for those of today.
const PAGE: usize = 64;

/// The runs a person approved on the local day of `now`, merged or merging, newest first. A run merged from the task
/// page or by Orchester's retry of it is approved by the person who asked, when it merged; one the merge queue merged,
/// when they approved it there. A run the Orchester merged by automation's policy, or before merge attempts were
/// recorded, was approved by no one this reads. A run the queue delivers after the approval is merging until it merges.
pub(super) fn approved_today(reader: &Reader<'_>, now: Timestamp) -> Result<Vec<ReviewRow>, StoreError> {
  let today = now.local_day_start();
  let mut rows = Vec::new();
  let mut listed = HashSet::new();
  for (task, merge) in merged_since(reader, today)? {
    let Some(approved) = approved_at(reader, &merge)? else {
      continue;
    };
    if approved >= today && listed.insert(merge.run) {
      rows.extend(row(
        reader,
        (task, merge.run),
        ReviewState::Merged { base: Some(merge.into) },
        approved,
      )?);
    }
  }
  for entry in reader.tasks.merge_queue.iter() {
    let Some(approved) = entry.approved_at.filter(|at| *at >= today && entry.state.in_queue()) else {
      continue;
    };
    if listed.insert(entry.run) {
      rows.extend(row(reader, (entry.task, entry.run), merging(entry), approved)?);
    }
  }
  rows.sort_by_key(|row| std::cmp::Reverse((row.since, row.run)));
  Ok(rows)
}

fn merging(entry: &QueueEntry) -> ReviewState {
  ReviewState::Merging {
    base: entry.base.clone(),
  }
}

fn row(
  reader: &Reader<'_>,
  (task, run): (TaskId, RunId),
  state: ReviewState,
  since: Timestamp,
) -> Result<Option<ReviewRow>, StoreError> {
  let (Some(task), Some(stored)) = (reader.task(task), reader.run(run)?) else {
    return Ok(None);
  };
  let threads = reader.db.review_threads(run)?;
  Ok(Some(reader.row(
    (task, (run, stored.number())),
    Some(&stored),
    &threads,
    (state, since),
  )))
}

/// The runs Orchester merged since `day` began, newest first.
fn merged_since(reader: &Reader<'_>, day: Timestamp) -> Result<Vec<(TaskId, RunMerge)>, StoreError> {
  let mut merged = Vec::new();
  let mut before = None;
  loop {
    let page = reader.db.merged_runs(before, PAGE)?;
    let full = page.len() == PAGE;
    before = page.last().map(|(_, merge)| (merge.merged_at, merge.run));
    let reached_before = page.iter().any(|(_, merge)| merge.merged_at < day);
    merged.extend(page.into_iter().filter(|(_, merge)| merge.merged_at >= day));
    if reached_before || !full {
      return Ok(merged);
    }
  }
}

/// When a person approved `merge`, which landed: see [`approved_today`]; `None` when no person did.
fn approved_at(reader: &Reader<'_>, merge: &RunMerge) -> Result<Option<Timestamp>, StoreError> {
  let attempts = reader.db.merge_attempts(merge.run)?;
  let Some(landed) = attempts.iter().find(|recorded| recorded.attempt.outcome().landed()) else {
    return Ok(None);
  };
  Ok(match landed.attempt.asked_by() {
    MergeAsker::Person | MergeAsker::Retry { .. } => Some(merge.merged_at),
    MergeAsker::MergeQueue => reader
      .db
      .merge_queue_entry(merge.run)?
      .and_then(|entry| entry.approved_at),
    MergeAsker::Automation => None,
  })
}
