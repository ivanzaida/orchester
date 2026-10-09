//! `live`: drafts ready to run with `orchester-fake-codex`, whose briefs say what the fake does in its turn
//! (`crates/runner/tests/support/fake_codex_work.rs`): ask to approve a command (Escalate), ask a question
//! with options, keep working until it is ended, fail the turn, or do the work and finish. Feature delivery
//! stops at its plan for a decision (`Plan` has plan approval); Quick fix is one Implement step, so a brief
//! that makes the fake ask asks once.

use crate::kontur::{Kontur, SeedError};

#[derive(Clone, Copy)]
enum Pipe {
  Feature,
  QuickFix,
}

/// Title, brief and pipeline, and what a run of the task does with the fake.
const TASKS: [(&str, &str, Pipe, &str); 6] = [
  (
    "Show the signed-in account",
    "Show who is signed in at the top of the sidebar.",
    Pipe::QuickFix,
    "draft; a run ends ready for review",
  ),
  (
    "Add serde to the webhook payloads",
    "Serialize the webhook payloads with serde. Escalate `cargo add serde`.",
    Pipe::QuickFix,
    "draft; a run asks to approve `cargo add serde`",
  ),
  (
    "Pick where the docs button goes",
    "Place the docs button where people look for help. Ask Codex with options.",
    Pipe::QuickFix,
    "draft; a run asks a question with options",
  ),
  (
    "Rate-limit messages from GitHub",
    "Respect GitHub's rate limits when the sync sends messages.",
    Pipe::Feature,
    "draft; a run waits for its plan to be decided",
  ),
  (
    "Typed errors per module",
    "Give every module its own error enum. Keep working.",
    Pipe::QuickFix,
    "draft; a run keeps working until it is stopped",
  ),
  (
    "Export invoices as PDF",
    "Export a month of invoices as one PDF per customer. Please fail.",
    Pipe::QuickFix,
    "draft; a run fails its turn",
  ),
];

const CRITERIA: [&str; 1] = ["The change is covered by tests."];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = kontur.live_catalogue()?;
  for (index, (title, brief, pipe, state)) in TASKS.into_iter().enumerate() {
    let created = kontur.ago(i64::try_from(TASKS.len() - index).expect("a short list") * 5);
    let pipeline = match pipe {
      Pipe::Feature => &catalogue.feature_delivery,
      Pipe::QuickFix => &catalogue.quick_fix,
    };
    let task = kontur.task((title, brief, &CRITERIA), Some(pipeline), created)?;
    kontur.seeded(&task, state);
  }
  Ok(())
}
