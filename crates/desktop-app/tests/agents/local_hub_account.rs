//! What a local profile runs on is this computer's choice (`Agents on accounts` C1 `RwzH2`, C2 `t7s3I`;
//! `docs/design/agent-accounts.md`, "Agents run on accounts", decision 4): the rail of the *Runs on* picker is this
//! computer's accounts; one the profile runs on as it is applies at once with no version, a model of another one is
//! held with it, and only a person moves a profile to another account at once.
//! `cargo test -p orchester-app --test agents local_hub_account`.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{HubFilter, HubView};
use orchester_core::{
  account::AccountCli,
  connection::{AgentConnection, AgentProvider, ConnectionAccount},
  id::{AgentAccountId, ConnectionId},
  name::Name,
  resource::VersionNumber,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::agents::data::VersionState;

use super::{
  harness::Agents,
  local_hub::{builder_without_connection, open},
  local_hub_profile::{
    ALLOW, FIELD, MENU, SAVE, builder_on_claude, claude, holds_a_change, on_rail, open_picker, passes, pick, runs_here,
    save, show, stored, v2,
  },
};

/// The rail's item of the Default account of `cli`.
fn default_of(cli: AccountCli) -> String {
  format!("connection-account-{}", cli.default_account())
}

#[test]
fn local_hub_account_an_account_chosen_on_the_rail_runs_it_there_at_once_with_no_version() {
  let mut agents = Agents::new(true);
  // Both connections are on Claude Code's Default account: the rail has the account once, and a choice of it runs
  // the profile on the older of them.
  let (profile, [claude_2, _]) = builder_without_connection(&agents);
  open(&mut agents, HubView::new(HubFilter::Profiles, Some(profile)));
  agents.until("the fix", |s| s.exists("local-hub-fix-use"));
  assert!(!agents.exists(SAVE), "while nothing is changed the bar is the fix's");
  assert!(
    agents
      .text("local-hub-profile-runs-on-row")
      .contains("Choose an account"),
    "{}",
    agents.text("local-hub-profile-runs-on-row")
  );

  pick(&mut agents, &default_of(AccountCli::ClaudeCode));

  agents.until("it runs on the account and is ready", |s| {
    runs_here(s, profile) == claude_2 && !s.exists("local-hub-fix-use")
  });
  assert!(!holds_a_change(&mut agents, profile), "nothing waits for Save");
  let (body, state) = stored(&agents, profile);
  assert_eq!(state, VersionState::Published(VersionNumber::FIRST), "no new version");
  assert_ne!(body.connection(), claude_2, "the version is as it was published");
  agents.until("the field names the account", |s| {
    s.text(FIELD).starts_with("Claude Code · Default ·")
  });
}

#[test]
fn local_hub_account_a_model_of_another_clis_connection_is_held_with_that_connection() {
  let mut agents = Agents::new(true);
  let connection = claude(&agents);
  let profile = builder_on_claude(&agents, connection);
  let codex = agents
    .listed()
    .connections
    .iter()
    .find(|found| found.provider() == AgentProvider::CodexCli)
    .expect("the project's Codex connection")
    .id();
  show(&mut agents, profile);

  // Codex does not serve `claude-a`: choosing its account on the rail only shows what it lists.
  pick(&mut agents, &default_of(AccountCli::Codex));
  passes(&mut agents);
  assert!(!holds_a_change(&mut agents, profile), "nothing is chosen yet");
  assert_eq!(runs_here(&agents, profile), connection);

  pick(&mut agents, "model-0");
  agents.until("the panel holds it", |s| holds_a_change(s, profile));
  // What Codex can enforce differs from Claude Code's, so the save may ask first: only a press answers.
  save(&mut agents, "published or its question", |s| {
    stored(s, profile).1 == v2() || s.exists(ALLOW)
  });
  if agents.exists(ALLOW) {
    agents.press_until(ALLOW, "published", |s| stored(s, profile).1 == v2());
  }

  let (body, _) = stored(&agents, profile);
  assert_eq!(body.connection(), codex, "it runs on the Codex connection now");
  assert_eq!(body.model().as_str(), "test-model", "on the model chosen there");
}

#[test]
fn local_hub_account_the_rail_is_this_computers_accounts_once_each_with_add_beside_the_field() {
  let mut agents = Agents::new(true);
  let (profile, connections) = builder_without_connection(&agents);
  open(&mut agents, HubView::new(HubFilter::Profiles, Some(profile)));
  agents.until("the panel", |s| s.exists(FIELD));

  open_picker(&mut agents);

  for cli in AccountCli::ALL {
    let item = format!("{MENU}-{}", default_of(cli));
    assert!(agents.exists(&item), "{item} is on the rail");
  }
  for connection in connections {
    let item = format!("{MENU}-{}", on_rail(connection));
    assert!(
      !agents.exists(&item),
      "{item}: a connection on a listed account is not an item of its own"
    );
  }
  assert!(agents.exists("local-hub-profile-add-connection"));
}

/// [`claude`]'s connection, and a second Claude Code connection on a named account of the person's that lists what
/// it does.
fn claude_and_work(agents: &Agents) -> (ConnectionId, ConnectionId) {
  let first = claude(agents);
  let db = ProjectDb::open(&agents.root).expect("database");
  let own = db
    .connection(first)
    .expect("read")
    .and_then(|stored| AgentConnection::try_from(stored).ok())
    .expect("the Claude connection");
  let mut work = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude Work").expect("name"),
    AgentProvider::ClaudeCodeCli,
    agents.root.join("claude.exe"),
    Timestamp::now(),
  )
  .expect("connection");
  work
    .set_account(ConnectionAccount::Account {
      id: AgentAccountId::generate(),
      folder: agents.root.clone(),
    })
    .expect("an agent CLI runs on an account");
  work
    .record_check(own.last_check().expect("checked").clone())
    .expect("check");
  let mut db = db;
  db.create_connection(&work).expect("stored");
  (first, work.id())
}

/// Work on another account goes to another sign-in: the choice has no *Save* that would ask, so an MCP client's press
/// does not make it, and a person's does.
#[test]
fn local_hub_account_only_a_person_moves_a_profile_to_another_account_at_once() {
  let mut agents = Agents::new(true);
  let (own, work) = claude_and_work(&agents);
  let profile = builder_on_claude(&agents, own);
  show(&mut agents, profile);
  open_picker(&mut agents);
  let item = format!("{MENU}-{}", on_rail(work));
  agents.until("the rail", |s| s.exists(&item));

  agents.until("the refusal", |s| {
    with_synthetic_input(|| s.click(&item));
    s.refused().is_some()
  });
  passes(&mut agents);
  assert_eq!(runs_here(&agents, profile), own, "automation moved nothing");

  agents.click(&item);
  agents.until("a person's press runs it on Work", |s| runs_here(s, profile) == work);
  assert_eq!(
    stored(&agents, profile).1,
    VersionState::Published(VersionNumber::FIRST),
    "with no new version"
  );
}

/// Choosing the connection a profile runs on is open to automation (the person's decision of 2026-11-11) when it
/// moves to no other account: an MCP client's *Use* runs a profile that ran on nothing on the first connection of its
/// kind, at once, as a person's does.
#[test]
fn local_hub_account_use_runs_it_on_the_first_of_its_kind_at_once_for_automation_too() {
  let mut agents = Agents::new(true);
  let (profile, [claude_2, _]) = builder_without_connection(&agents);
  open(&mut agents, HubView::new(HubFilter::Profiles, Some(profile)));
  agents.until("the fix", |s| s.exists("local-hub-fix-use"));
  assert!(
    agents
      .text("local-hub-fix-use")
      .contains("Use Claude Code · Default for Builder"),
    "the account, not a connection: {}",
    agents.text("local-hub-fix-use")
  );

  with_synthetic_input(|| agents.click("local-hub-fix-use"));

  agents.until("it runs on Claude 2 and is ready", |s| {
    runs_here(s, profile) == claude_2 && !s.exists("local-hub-fix-use")
  });
  assert!(!holds_a_change(&mut agents, profile), "nothing waits for Save");
  assert_eq!(
    stored(&agents, profile).1,
    VersionState::Published(VersionNumber::FIRST),
    "with no new version"
  );
  assert!(!agents.exists(ALLOW), "nothing to ask: no other account");
}
