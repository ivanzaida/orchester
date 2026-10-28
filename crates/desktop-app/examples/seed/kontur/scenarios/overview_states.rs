//! `overview-states`: the Overview's states sheet (05 `JzNRO`) on the `tasks` project. Seven more tasks wait for the
//! person (eleven in all) and four more run (seven at work), so *Waiting for you* and *At work* show *Show all N*; four
//! more are queued, so *Queue* does too. Three Claude Code and Codex accounts carry plan readings: *Work* near its
//! 5-hour limit (86 %), *Personal* at it, and *Client*'s reading near its limit before its window reset an hour ago,
//! which shows greyed as *unknown since reset*. The accounts' folders are names only: nothing runs on them.

use std::path::{Path, PathBuf};

use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount},
  connection::plan_limits::{LimitState, LimitWindow, PlanLimits},
  id::AgentAccountId,
  time::Timestamp,
};
use orchester_store::AppDb;

use super::tasks;
use crate::kontur::{
  Kontur, SeedError, name,
  shapes::{self, Shape},
};

/// The 5-hour window, in minutes.
const FIVE_HOURS: u64 = 300;
const CRITERIA: [&str; 2] = ["The change is covered by tests.", "Nothing changes for existing users."];

/// The tasks added to `tasks`', each with where it is left.
const ADDED: [(&str, Shape); 15] = [
  ("Retry failed CSV rows", Shape::Question("implement")),
  ("Warn before a webhook secret expires", Shape::Approval("implement")),
  ("Plan the audit log export", Shape::PlanReview),
  ("Archive old invoices", Shape::Stuck("run-checks")),
  ("Throttle the HubSpot sync", Shape::Interrupted("implement")),
  ("Map HubSpot owners to ours", Shape::Question("plan")),
  (
    "Send a weekly usage digest",
    Shape::AtLimit("implement", orchester_core::run::RunLimit::Tokens),
  ),
  ("Show the sync's last error", Shape::Running("implement")),
  ("Add a health endpoint", Shape::Running("plan")),
  ("Log slow queries", Shape::Running("review")),
  ("Cache the contact search", Shape::Running("implement")),
  ("Export contacts as CSV", Shape::Queued),
  ("Retry GitHub webhooks", Shape::Queued),
  ("Reword the sign-in errors", Shape::Queued),
  ("Trim the event payloads", Shape::Queued),
];

/// One account and its plan reading: name, CLI, 5-hour use, whether the limit is reached, and when it was read and
/// resets, in minutes before now (a negative reset is after now).
struct Planned {
  cli: AccountCli,
  named: &'static str,
  used: f64,
  reached: bool,
  read_ago: i64,
  resets_ago: i64,
}

const ACCOUNTS: [Planned; 3] = [
  Planned {
    cli: AccountCli::ClaudeCode,
    named: "Work",
    used: 86.0,
    reached: false,
    read_ago: 5,
    resets_ago: -100,
  },
  Planned {
    cli: AccountCli::ClaudeCode,
    named: "Personal",
    used: 100.0,
    reached: true,
    read_ago: 10,
    resets_ago: -80,
  },
  Planned {
    cli: AccountCli::Codex,
    named: "Client",
    used: 91.0,
    reached: false,
    read_ago: 240,
    resets_ago: 60,
  },
];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let catalogue = tasks::project(kontur)?;
  let feature = &catalogue.feature_delivery;
  for (index, (title, shape)) in ADDED.into_iter().enumerate() {
    let ago = 200 - 10 * i64::try_from(index).expect("a few tasks");
    let (created, started) = (kontur.ago(ago), kontur.ago(ago - 5));
    let mut task = kontur.task((title, "added for the states sheet", &CRITERIA), Some(feature), created)?;
    shapes::apply(kontur, &mut task, Some(feature), shape, started)?;
  }
  accounts(kontur)
}

/// Adds [`ACCOUNTS`] to the application database with their readings, as reads outside a run keep them.
fn accounts(kontur: &Kontur) -> Result<(), SeedError> {
  let folders = kontur
    .app_database
    .parent()
    .map_or_else(|| PathBuf::from("accounts"), |folder| folder.join("accounts"));
  let mut db = AppDb::open(&kontur.app_database)?;
  for planned in &ACCOUNTS {
    let read = kontur.ago(planned.read_ago);
    let account = add_account(&mut db, &folders, planned, read)?;
    let state = if planned.reached {
      LimitState::Reached
    } else {
      LimitState::Normal
    };
    let resets = kontur.ago(planned.resets_ago);
    let window = LimitWindow::new(FIVE_HOURS, Some(planned.used), Some(resets), state, false)
      .map_err(|error| SeedError::Invalid(error.to_string()))?;
    let limits = PlanLimits {
      windows: vec![window],
      plan: None,
      credits: None,
      blocked: None,
    };
    db.save_plan_read((account.id(), planned.cli), read, &limits)?;
  }
  Ok(())
}

fn add_account(db: &mut AppDb, folders: &Path, planned: &Planned, at: Timestamp) -> Result<AgentAccount, SeedError> {
  let account = AgentAccount::new(
    AgentAccountId::generate(),
    planned.cli,
    name(planned.named),
    AccountFolder::Created(folders.join(planned.named.to_lowercase())),
    at,
  )
  .map_err(|error| SeedError::Invalid(error.to_string()))?;
  db.add_agent_account(&account)?;
  Ok(account)
}
