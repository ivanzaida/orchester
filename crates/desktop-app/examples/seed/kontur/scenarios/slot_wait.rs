//! `slot-wait`: one agent slot and two drafts, ready to run with `orchester-fake-codex`, one at a time. A run of the
//! first, on *Review and verify*, passes Implement, then Review and Verify are ready together and Review, the first,
//! gets the slot: it keeps working until it is ended, and Verify waits for an agent slot, with Report behind them still
//! not started. A run of the second, on *Review, verify and test*, has Verify and Test waiting, for the words of two
//! steps and the health card's value on two lines. Raising *Agents at once* ends the wait.

use orchester_core::{
  limits::{AgentSlots, ProjectLimits},
  store::SettingsStore,
};

use crate::kontur::{Kontur, SeedError};

const TITLE: &str = "Compare the export's totals";
const BRIEF: &str = "Compare the totals of a month's export with the invoices it holds.";
const TWO_WAIT_TITLE: &str = "Compare the refunds' totals";
const TWO_WAIT_BRIEF: &str = "Compare the totals of a month's refunds with the credit notes they hold.";
const CRITERIA: [&str; 1] = ["The change is covered by tests."];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let (one_waits, two_wait) = kontur.slot_wait_catalogue()?;
  let one = AgentSlots::new(1).expect("one is in range");
  kontur
    .db
    .save_project_limits(&ProjectLimits::DEFAULT.with_agent_slots(one))?;
  let created = kontur.ago(5);
  let task = kontur.task((TITLE, BRIEF, &CRITERIA), Some(&one_waits), created)?;
  kontur.seeded(
    &task,
    "draft; a run's Review works and its Verify waits for the one agent slot",
  );
  let created = kontur.ago(4);
  let task = kontur.task((TWO_WAIT_TITLE, TWO_WAIT_BRIEF, &CRITERIA), Some(&two_wait), created)?;
  kontur.seeded(
    &task,
    "draft; a run's Review works and its Verify and Test wait for the one agent slot",
  );
  Ok(())
}
