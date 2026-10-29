//! `settings`: the accounts in their states with the stand-in CLIs beside the database, plan readings, quiet hours and
//! the daily report, as app Settings' QA reads them (ORC-104).

use orchester_core::account::{AccountCli, AccountState};
use orchester_store::AppDb;

use super::{Seed, kontur::scenarios::settings};

#[test]
fn seed_settings_writes_accounts_in_their_states_and_the_notifications() {
  let seed = Seed::of("settings");
  let db = AppDb::open(&seed.seeded.app_database).expect("the application database");
  let accounts = db.agent_accounts().expect("the accounts");
  let state = |name: &str| {
    accounts
      .iter()
      .find(|account| account.name().is_some_and(|known| known.as_str() == name))
      .map(|account| account.state())
      .unwrap_or_else(|| panic!("no account {name}"))
  };
  assert_eq!(state("Work"), AccountState::SignedIn);
  assert_eq!(state("Personal"), AccountState::SignedIn);
  assert_eq!(state("Old"), AccountState::Expired);
  assert_eq!(state("Client"), AccountState::SignedIn);
  assert_eq!(state("Archive"), AccountState::FolderMissing);
  let defaults = accounts
    .iter()
    .filter(|account| account.name().is_none())
    .map(|account| account.cli())
    .collect::<Vec<_>>();
  assert!(
    defaults.contains(&AccountCli::ClaudeCode) && defaults.contains(&AccountCli::Codex),
    "both Defaults keep their checks"
  );

  let notifications = db.notification_settings().expect("the notifications");
  let quiet = notifications.quiet.expect("quiet hours on");
  assert_eq!((quiet.start(), quiet.end()), (22 * 60 + 30, 7 * 60));
  let report = db.daily_report_schedule().expect("the report").expect("the report on");
  assert_eq!(report.time.minute(), 18 * 60 + 30);

  let stand_ins = settings::folder(&seed.seeded.app_database).join("path");
  assert!(
    std::fs::read_dir(&stand_ins).expect("the stand-in CLIs").count() >= 2,
    "claude and codex stand-ins"
  );
}
