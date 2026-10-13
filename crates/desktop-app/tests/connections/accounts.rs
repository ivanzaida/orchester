//! A connection's account (`docs/design/agent-accounts.md`, `M42rM`): the list and the inspector name the account a
//! CLI connection runs on, the inspector as a link to it in Settings, and choosing another one in Edit connection
//! stores it and checks the connection there.

use orchester_app::routes::{ConnectionsTab, ConnectionsView, ProjectPage};
use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount},
  connection::{AgentProvider, ConnectionAccount, ConnectionKind},
  id::AgentAccountId,
  name::Name,
  store::ConnectionStore,
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
  seeded.until("the inspector to say so", |seeded| {
    seeded.text("connection-account-row").contains("Client")
  });
}
