//! Project settings › Memory as Context's Memory group saves it: what a save changed is rebased onto the stored
//! settings, the limits are read as typed, and the Rejected tab lists the withdrawn proposals too. Run with
//! `cargo test -p orchester-workspace --test memory_settings memory_settings`.

mod support;

use orchester_core::memory::{MemoryLimits, MemorySettings, MemoryStatus, MemoryStore, WritePolicy};
use orchester_workspace::memory::{listing::StatusFilter, settings};
use support::Fixture;

#[test]
fn memory_settings_a_save_keeps_what_changed_elsewhere_meanwhile() {
  let mut fixture = Fixture::new();
  let base = MemorySettings::DEFAULT;
  let elsewhere = MemorySettings {
    limits: MemoryLimits::new(12, 3_000).expect("valid limits"),
    ..base
  };
  fixture.db.save_memory_settings(&elsewhere).expect("saved elsewhere");
  let asked = MemorySettings {
    project_writes: WritePolicy::Automatic,
    limits: MemoryLimits::new(8, 5_000).expect("valid limits"),
    ..base
  };
  let saved = settings::save_in(fixture.repo.path(), base, asked).expect("saved");
  assert_eq!(saved.project_writes, WritePolicy::Automatic);
  assert_eq!(saved.task_writes, WritePolicy::Automatic, "untouched");
  assert_eq!(
    (saved.limits.records(), saved.limits.tokens()),
    (12, 5_000),
    "the records changed elsewhere stay; the tokens asked for are saved"
  );
  assert_eq!(settings::load_in(fixture.repo.path()).expect("read back"), saved);
}

#[test]
fn memory_settings_limits_are_read_as_typed() {
  assert_eq!(settings::read_records(" 12 "), Ok(12));
  assert_eq!(settings::read_tokens("5,000", ","), Ok(5_000));
  assert_eq!(settings::read_tokens("20000", ","), Ok(20_000));
  let records = settings::read_records("51").expect_err("past the range");
  assert_eq!(records.key, "memory.settings.recordsInvalid");
  let tokens = settings::read_tokens("1.5k", ",").expect_err("not a whole number");
  assert_eq!(tokens.key, "memory.settings.tokensInvalid");
  assert!(settings::read_tokens("499", ",").is_err());
}

#[test]
fn memory_settings_rejected_tab_lists_withdrawn_proposals_too() {
  let rejected = StatusFilter::RejectedOrWithdrawn;
  assert!(rejected.shows(MemoryStatus::Rejected) && rejected.shows(MemoryStatus::Withdrawn));
  assert!(!rejected.shows(MemoryStatus::Proposed) && !rejected.shows(MemoryStatus::Active));
  assert!(
    !StatusFilter::CHOICES.contains(&rejected),
    "not a choice of the task tab's Status filter"
  );
}
