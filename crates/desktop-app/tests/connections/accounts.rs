//! A connection's account (`docs/design/agent-accounts.md`, `M42rM`): the list and the inspector name the account a
//! CLI connection runs on, the inspector as a link to it in Settings, and choosing another one in Edit connection
//! stores it and checks the connection there.

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
  seeded.list();
  let row = format!("connection-row-{}-row", codex.id());
  seeded.until("its row", |seeded| seeded.exists(&row));
  assert!(seeded.text(&row).contains("account Default"));
  seeded.open(codex.id());
  seeded.until("the account row", |seeded| seeded.exists("connection-account-row-row"));
  assert!(seeded.text("connection-account-row-row").contains("Default"));
  assert!(
    seeded.exists("connection-account-link"),
    "the account opens in Settings"
  );
  seeded.click("connection-more");
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
  // The page reads the connection again after Save: its account row is gone for a moment.
  seeded.until("the page to say so", |seeded| {
    seeded.exists("connection-account-row-row") && seeded.text("connection-account-row-row").contains("Client")
  });
}

/// Freelance, a Codex account its CLI says is signed out, and a Codex connection on it.
fn signed_out_freelance(seeded: &Seeded) -> (AgentAccount, ConnectionId) {
  signed_out(seeded, "Freelance")
}

/// An account named `account` its CLI says is signed out, and a Codex connection on it.
fn signed_out(seeded: &Seeded, account: &str) -> (AgentAccount, ConnectionId) {
  let folder = seeded.window.data.dir.path().join("codex-freelance");
  std::fs::create_dir_all(&folder).expect("the account's folder");
  let mut freelance = AgentAccount::new(
    AgentAccountId::generate(),
    AccountCli::Codex,
    Name::new(account).expect("a name"),
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
    Name::new(&format!("Codex · {account}")).expect("a name"),
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
  seeded.open(id);
  seeded.until("the account row", |seeded| seeded.exists("connection-account-link"));
  assert!(seeded.text("connection-account-link").contains("Freelance"));
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
  seeded.click("connection-more");
  assert!(seeded.exists("connection-edit"), "Edit connection in the … menu");
  seeded.click("connection-more");
  seeded.until("its plan limits", |seeded| {
    seeded
      .window
      .texts("connection-account-row-hint")
      .is_some_and(|hint| hint.join(" ").contains("5-hour 12% · weekly 41%"))
  });
}

/// The bounds of `id` as laid out.
fn bounds(seeded: &mut Seeded, id: &str) -> lurq::core::ElementRect {
  seeded
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

#[test]
fn connections_signed_out_row_keeps_its_button_inside_it_whatever_the_account_name() {
  for account in [
    "Freelance",
    "Freelance work for the northern region clients and their many projects",
  ] {
    let mut seeded = Seeded::new("accounts");
    let (_, id) = signed_out(&seeded, account);
    seeded.open(id);
    seeded.until("Open in Settings", |seeded| seeded.exists("connection-open-account"));
    let row = bounds(&mut seeded, "connection-inspector-sentence-row");
    let inside = bounds(&mut seeded, "connection-open-account");
    assert!(
      inside.x >= row.x && inside.x + inside.width <= row.x + row.width + 0.5,
      "Open in Settings of {account} at {inside:?} runs past its row {row:?}"
    );
  }
}

#[test]
fn connections_on_no_account_say_so_whatever_their_last_check() {
  let mut seeded = Seeded::new("accounts");
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Codex · Left").expect("a name"),
    AgentProvider::CodexCli,
    seeded.window.data.dir.path().join("codex.cmd"),
    Timestamp::now(),
  )
  .expect("a connection");
  connection
    .set_account(ConnectionAccount::Unassigned)
    .expect("a CLI connection");
  // A check an older Orchester made on Default's folder after the account went.
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "0.159.2".to_owned(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
      },
    })
    .expect("its kind");
  let id = connection.id();
  seeded
    .db()
    .create_connection(&connection.into_connection())
    .expect("created");
  seeded.list();
  let row = format!("connection-row-{id}-row");
  seeded.until("its row", |seeded| seeded.exists(&row));
  let text = seeded.text(&row);
  assert!(text.contains("No account") && !text.contains("Connected"), "{text}");
  seeded.open(id);
  let text = seeded.text("connection-inspector-header");
  assert!(text.contains("No account") && !text.contains("Connected"), "{text}");
  assert!(
    seeded
      .text("connection-inspector-sentence")
      .contains("runs on no account")
  );
  seeded.click("connection-more");
  assert!(seeded.exists("connection-edit"));
}
