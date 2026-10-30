//! `slot-wait`: one agent slot and a draft on *Review and verify*, ready to run with `orchester-fake-codex`. A run of
//! it passes Implement, then Review and Verify are ready together and Review, the first, gets the slot: it keeps
//! working until it is ended, and Verify waits for an agent slot, with Report behind them still not started. Raising
//! *Agents at once* to 2 ends the wait.

use orchester_core::{
  limits::{AgentSlots, ProjectLimits},
  store::SettingsStore,
};

use crate::kontur::{Kontur, SeedError};

const TITLE: &str = "Compare the export's totals";
const BRIEF: &str = "Compare the totals of a month's export with the invoices it holds.";
const CRITERIA: [&str; 1] = ["The change is covered by tests."];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let pipeline = kontur.slot_wait_catalogue()?;
  let one = AgentSlots::new(1).expect("one is in range");
  kontur
    .db
    .save_project_limits(&ProjectLimits::DEFAULT.with_agent_slots(one))?;
  let created = kontur.ago(5);
  let task = kontur.task((TITLE, BRIEF, &CRITERIA), Some(&pipeline), created)?;
  kontur.seeded(
    &task,
    "draft; a run's Review works and its Verify waits for the one agent slot",
  );
  Ok(())
}
