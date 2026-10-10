//! `lifecycle`: drafts ready to run with `orchester-fake-codex` for closing the window while agents work
//! (layer "App lifecycle" `Ob1P7`): one keeps working until it is ended, one works until the test (or a
//! person) writes `.orchester/fake-release` in the repository and then saves at its checkpoint (its first step of
//! several, so the run pauses there), and one stops at its plan for a decision, so it only waits for a person
//! (`crates/runner/tests/support/fake_codex_work.rs`).

use crate::kontur::{Kontur, SeedError};

/// Title, brief, whether it runs Feature delivery (with a plan to decide) rather than Quick fix, and what a
/// run of it does with the fake.
const TASKS: [(&str, &str, bool, &str); 3] = [
  (
    "Typed errors per module",
    "Give every module its own error enum. Keep working.",
    false,
    "draft; a run keeps working until it is ended",
  ),
  (
    "Save at a checkpoint",
    "Run the tests. Wait for release.",
    true,
    "draft; a run's first step works until .orchester/fake-release exists, then saves",
  ),
  (
    "Rate-limit messages from GitHub",
    "Respect GitHub's rate limits when the sync sends messages.",
    true,
    "draft; a run waits for its plan to be decided",
  ),
];

const CRITERIA: [&str; 1] = ["The change is covered by tests."];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.live_catalogue()?;
  for (index, (title, brief, feature, state)) in TASKS.into_iter().enumerate() {
    let created = kontur.ago(i64::try_from(TASKS.len() - index).expect("a short list") * 5);
    let pipeline = if feature {
      &catalogue.feature_delivery
    } else {
      &catalogue.quick_fix
    };
    let task = kontur.task((title, brief, &CRITERIA), Some(pipeline), created)?;
    kontur.seeded(&task, state);
  }
  Ok(())
}
