//! `accounts`: what the screens that show agent accounts need (`docs/design/agent-accounts.md`, layer `hgOl2`), and
//! no unreadable run, so every page of the project reads. KON-1 is stopped at its account's plan limit (`EwAhH`),
//! KON-2 runs its Implement step, KON-3 is a new run waiting for its account's reset before Research starts
//! (`XPjSc`), KON-4 is ready for review. Runs use Feature delivery on the project's Codex
//! connection; the accounts themselves are the application's, which `examples/accounts_qa` adds.

use orchester_core::time::Timestamp;

use crate::kontur::{
  Kontur, SeedError,
  shapes::{self, Shape},
};

const STATES: [(&str, &str, Shape); 4] = [
  (
    "Batch webhook deliveries",
    "Send webhook deliveries to one endpoint in batches.",
    Shape::AtPlanLimit("implement"),
  ),
  (
    "Add retry to webhook sender",
    "Retry failed webhook deliveries with backoff and jitter.",
    Shape::Running("implement"),
  ),
  (
    "Trim old deliveries",
    "Delete deliveries older than 30 days.",
    Shape::WaitsForPlanLimit("research"),
  ),
  (
    "Show the signed-in account",
    "Show who is signed in at the top of the sidebar.",
    Shape::Succeeded,
  ),
];
const CRITERIA: [&str; 2] = ["The change is covered by tests.", "Nothing changes for existing users."];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.catalogue()?;
  for (index, (title, brief, shape)) in STATES.into_iter().enumerate() {
    let hours = i64::try_from(STATES.len() - index).expect("a short list") * 60;
    let started = kontur.ago(hours);
    let created = Timestamp::from_unix_millis(started.unix_millis() - 30 * 60_000);
    let pipeline = Some(&catalogue.feature_delivery);
    let mut task = kontur.task((title, brief, &CRITERIA), pipeline, created)?;
    shapes::apply(kontur, &mut task, pipeline, shape, started)?;
  }
  Ok(())
}
