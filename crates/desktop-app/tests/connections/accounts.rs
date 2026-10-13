//! A connection's account (`docs/design/agent-accounts.md`, `M42rM`): the list and the inspector name the account a
//! CLI connection runs on, the inspector as a link to it in Settings, and choosing another one in Edit connection
//! stores it and checks the connection there.

use orchester_app::routes::{ConnectionsTab, ConnectionsView, ProjectPage};
use orchester_core::{
  account::{AccountCheck, AccountCheckResult, AccountCli, AccountFolder, AgentAccount, CheckedProgram},
  connection::{
    AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionAccount, ConnectionCheck, ConnectionKind,
    ModelCatalog, SignIn,
    plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits},
  },
  id::{AgentAccountId, ConnectionId},
  name::Name,
  store::{ConnectionStore, RunStore},
  time::Timestamp,
};
use orchester_store::AppDb;

use super::harness::Seeded;

#[test]
fn connections_name_their_account_and_a_person_chooses_another_in_edit_connection() {
  let mut seeded = Seeded::new("connections");
  let folder = seeded.window.data.dir.path().join("codex-client");
  std::fs::create_dir_all(&folder).expect("the account's folder");
  let client = AgentAccount::new(
    AgentAccountId::generate(),
    AccountCli::Codex,
    Name::new("Client").expect("a name"),
    AccountFolder::Created(folder),
    Timestamp::now(),
  )
  .expect("an account");
  AppDb::open(&seeded.window.data.database())
    .expect("the application database")
    .add_agent_account(&client)
    .expect("added");
  let codex = seeded
    .db()
    .connections()
    .expect("connections")
    .into_iter()
    .find(|connection| connection.kind() == &ConnectionKind::Agent(AgentProvider::CodexCli))
    .expect("the seed's Codex connection");
  let view = ConnectionsView::new(ConnectionsTab::AgentProviders).with_selected(codex.id());
  seeded.go(ProjectPage::Connections(view));
  seeded.until("the account row", |seeded| seeded.exists("connection-account-row"));
  assert!(seeded.text("connection-account-row").contains("Default"));
  assert!(
    seeded
      .text(&format!("connection-row-{}", codex.id()))
      .contains("account Default")
  );
  assert!(
    seeded.exists("connection-account-link"),
    "the account opens in Settings"
  );
  seeded.click("connection-edit");
  seeded.until("Edit connection", |seeded| seeded.exists("connection-account"));
  seeded.click("connection-account");
  seeded.until("the accounts menu", |seeded| {
    seeded.exists("connection-account-option-0")
  });
  let client_option = (0..4)
    .map(|index| format!("connection-account-option-{index}"))
    .find(|id| seeded.exists(id) && seeded.text(id).contains("Client"))
    .expect("Client is offered");
  assert!(
    seeded.text("connection-account-option-0").contains("Client"),
    "Default comes last"
  );
  seeded.click(&client_option);
  seeded.until("Save", |seeded| seeded.exists("connection-edit-save"));
  seeded.click("connection-edit-save");
  seeded.until("the connection on Client", |seeded| {
    seeded
      .db()
      .connection(codex.id())
      .expect("read")
      .is_some_and(|connection| connection.account() == &ConnectionAccount::of(&client))
  });
  // The inspector reads the connection again after Save: its account row is gone for a moment.
  seeded.until("the inspector to say so", |seeded| {
    seeded.exists("connection-account-row") && seeded.text("connection-account-row").contains("Client")
  });
}

/// Freelance, a Codex account its CLI says is signed out, and a Codex connection on it.
fn signed_out_freelance(seeded: &Seeded) -> (AgentAccount, ConnectionId) {
  let folder = seeded.window.data.dir.path().join("codex-freelance");
  std::fs::create_dir_all(&folder).expect("the account's folder");
  let mut freelance = AgentAccount::new(
    AgentAccountId::generate(),
    AccountCli::Codex,
    Name::new("Freelance").expect("a name"),
    AccountFolder::Created(folder.clone()),
    Timestamp::now(),
  )
  .expect("an account");
  freelance.record_check(AccountCheck {
    at: Timestamp::now(),
    result: AccountCheckResult::SignedOut {
      program: CheckedProgram {
        path: folder.join("codex.cmd"),
        version: "0.159.2".to_owned(),
      },
      was: None,
    },
  });
  let mut app = AppDb::open(&seeded.window.data.database()).expect("the application database");
  app.add_agent_account(&freelance).expect("added");
  app.save_agent_account(&freelance).expect("its check");
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Codex · Freelance").expect("a name"),
    AgentProvider::CodexCli,
    folder.join("codex.cmd"),
    Timestamp::now(),
  )
  .expect("a connection");
  connection
    .set_account(ConnectionAccount::of(&freelance))
    .expect("a Codex account");
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "0.159.2".to_owned(),
        sign_in: SignIn::SignedOut,
        catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
      },
    })
    .expect("its kind");
  let id = connection.id();
  seeded
    .db()
    .create_connection(&connection.into_connection())
    .expect("created");
  (freelance, id)
}

/// A running run of the project reports `account`'s windows: 5-hour 12%, weekly 41%.
fn report_windows(seeded: &Seeded, account: &AgentAccount) {
  let run = seeded
    .db()
    .running_runs()
    .expect("runs")
    .into_iter()
    .filter_map(Result::ok)
    .next()
    .expect("a running run");
  let reading = LimitReading {
    provider: AgentProvider::CodexCli,
    account: Some(account.id()),
    at: Timestamp::now(),
    limits: PlanLimits {
      windows: vec![
        LimitWindow::new(300, Some(12.0), None, LimitState::Normal, false).expect("a window"),
        LimitWindow::new(10_080, Some(41.0), None, LimitState::Normal, false).expect("a window"),
      ],
      plan: None,
      credits: None,
      blocked: None,
    },
  };
  seeded.db().save_limit_snapshot(run.id(), &reading).expect("stored");
}

#[test]
fn connections_on_a_signed_out_account_send_the_person_to_settings_and_show_its_plan_limits() {
  let mut seeded = Seeded::new("accounts");
  let (freelance, id) = signed_out_freelance(&seeded);
  report_windows(&seeded, &freelance);
  let view = ConnectionsView::new(ConnectionsTab::AgentProviders).with_selected(id);
  seeded.go(ProjectPage::Connections(view));
  seeded.until("the account row", |seeded| seeded.exists("connection-account-link"));
  assert!(
    seeded
      .text("connection-account-link")
      .contains("Freelance · not signed in")
  );
  seeded.until("Open in Settings", |seeded| seeded.exists("connection-open-account"));
  assert!(
    seeded
      .text("connection-open-account")
      .contains("Open Freelance in Settings")
  );
  assert!(
    !seeded.exists("connection-sign-in-command"),
    "the fix is in Settings, not a command here"
  );
  assert!(seeded.exists("connection-edit"));
  seeded.until("its plan limits", |seeded| seeded.exists("connection-plan-limits"));
  assert!(
    seeded
      .text("connection-plan-limits")
      .contains("5-hour 12% · weekly 41%")
  );
}
