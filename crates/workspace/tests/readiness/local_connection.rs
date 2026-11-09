//! The connection of a local profile (`Agents · v2` L1, ORC-842): a profile whose connection is gone names the CLI its
//! model belongs to and the connections of that CLI to choose from; choosing one publishes the next version on it.

use orchester_core::{
  connection::{AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, ConnectionAccount, EndpointPreset},
  id::{AgentAccountId, ConnectionId, ResourceId, SecretId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::{AgentProfile, Draft, ModelId, PromptText, ProviderSettings, RepositoryAccess, VersionNumber},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_workspace::{
  agents::{local_connection::use_connection, local_hub::LocalHub},
  readiness::{Group, ReadinessStanding, Undecided},
};

use crate::support::{Fixture, at, name};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;

fn hub(fixture: &Fixture) -> LocalHub {
  fixture
    .workspace
    .local_hub(&fixture.root, &fixture.vault)
    .expect("the hub")
}

fn builder_on(connection: ConnectionId) -> AgentProfile {
  AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    connection,
    ModelId::new("claude-opus-5-5").expect("model"),
    ProviderSettings::default(),
  )
  // What every CLI enforces on every platform, so a publication is not refused for its permissions.
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AskBefore,
    NetworkAccess::Off,
  ))
}

/// Builder on a connection this project no longer has, with two Claude Code connections and a Codex one.
fn gone() -> (Fixture, ResourceId, [ConnectionId; 3]) {
  let fixture = Fixture::local();
  let claude_2 = fixture.connection(CLAUDE, "claude-2");
  let home = fixture.connection(CLAUDE, "home");
  let codex = fixture.connection(AgentProvider::CodexCli, "codex-main");
  let builder = fixture.own("builder", &builder_on(ConnectionId::generate()));
  (fixture, builder, [claude_2, home, codex])
}

#[test]
fn readiness_local_connection_a_gone_connection_names_the_cli_of_the_model_and_its_connections() {
  let (fixture, builder, [claude_2, home, _]) = gone();

  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
  assert_eq!(sheet.cli, Some(CLAUDE), "the model says which CLI it was for");
  assert_eq!(sheet.connection, None);
  assert_eq!(sheet.current, None);
  assert_eq!(
    sheet.connections.iter().map(|choice| choice.id).collect::<Vec<_>>(),
    [claude_2, home],
    "only the connections of its CLI are offered"
  );
  let found = read
    .readiness()
    .resources()
    .iter()
    .find(|found| found.resource() == builder)
    .expect("listed");
  assert_eq!(found.group(), Group::NeedsYouHere);
  assert!(matches!(
    found.standing(),
    ReadinessStanding::Undecided(Undecided::ConnectionGone(_))
  ));
}

#[test]
fn readiness_local_connection_using_one_publishes_the_next_version_on_it() {
  let (fixture, builder, [claude_2, ..]) = gone();

  let version = use_connection(&fixture.root, builder, claude_2).expect("chosen");

  assert_eq!(version.get(), 2, "a new version, not an edit of v1");
  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
  assert_eq!(sheet.current, Some(claude_2));
  assert_eq!(sheet.connection.as_deref(), Some("claude-2"));
  assert!(!sheet.draft, "nothing is left unpublished");
  let found = read
    .readiness()
    .resources()
    .iter()
    .find(|found| found.resource() == builder)
    .expect("listed");
  assert_eq!(found.number(), VersionNumber::FIRST.next().expect("v2"));
  assert_eq!(found.group(), Group::Ready);
}

#[test]
fn readiness_local_connection_refuses_another_cli_and_unpublished_edits_and_changes_nothing() {
  let (fixture, builder, [claude_2, _, codex]) = gone();

  let other = use_connection(&fixture.root, builder, codex).expect_err("a Codex connection does not run Claude Code");
  assert_eq!(other.key, "agents.localConnection.notOfKind");

  fixture
    .db()
    .save_draft(
      builder,
      &Draft::new(builder_on(ConnectionId::generate()), Some(VersionNumber::FIRST), at(20)),
    )
    .expect("a draft");
  let held = use_connection(&fixture.root, builder, claude_2).expect_err("edits wait");
  assert_eq!(held.key, "agents.localConnection.draftFirst");

  let read = hub(&fixture);
  assert_eq!(
    read.profile(builder).expect("a sheet").current,
    None,
    "nothing was published"
  );
}

/// A Claude Code connection on a named account of the person's.
fn on_account(fixture: &Fixture, label: &str) -> ConnectionId {
  let mut made = AgentConnection::new(
    ConnectionId::generate(),
    name(label),
    CLAUDE,
    fixture.root.join(format!("{label}.exe")),
    Timestamp::now(),
  )
  .expect("connection");
  made
    .set_account(ConnectionAccount::Account {
      id: AgentAccountId::generate(),
      folder: fixture.root.clone(),
    })
    .expect("an agent CLI runs on an account");
  fixture.db().create_connection(&made).expect("a connection");
  made.id()
}

#[test]
fn readiness_local_connection_a_gone_connection_can_be_replaced_by_one_on_a_named_account() {
  let (fixture, builder, _) = gone();
  let work = on_account(&fixture, "work");

  let version = use_connection(&fixture.root, builder, work).expect("nothing moves away from an account that is gone");

  assert_eq!(version.get(), 2);
  assert_eq!(hub(&fixture).profile(builder).expect("a sheet").current, Some(work));
}

#[test]
fn readiness_local_connection_moving_from_a_connection_that_is_there_to_another_account_still_asks() {
  let fixture = Fixture::local();
  let home = fixture.connection(CLAUDE, "home");
  let work = on_account(&fixture, "work");
  let builder = fixture.own("builder", &builder_on(home));

  let refused = use_connection(&fixture.root, builder, work).expect_err("another account is a person's decision");

  assert_eq!(refused.key, "agents.localConnection.widens");
  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
  assert_eq!(sheet.current, Some(home), "still on the connection it had");
  assert!(!sheet.draft, "the refused save left no draft behind");
}

#[test]
fn readiness_local_connection_a_publication_that_is_refused_leaves_no_draft_behind() {
  let fixture = Fixture::local();
  let native = |label: &str| {
    AgentConnection::native(
      ConnectionId::generate(),
      name(label),
      AgentProvider::OpenAiApi,
      ApiEndpoint::new(
        EndpointPreset::Official,
        BaseUrl::new("https://api.openai.com").expect("a URL"),
        SecretId::generate(),
      ),
      Timestamp::now(),
    )
    .expect("a native connection")
  };
  let (first, second) = (native("openai-1"), native("openai-2"));
  fixture.db().create_connection(&first).expect("stored");
  fixture.db().create_connection(&second).expect("stored");
  let builder = fixture.own("builder", &builder_on(first.id()));

  // Saving a draft on it is fine; publishing a step that runs on it is refused: this version has no client for it.
  let refused = use_connection(&fixture.root, builder, second.id()).expect_err("no publication on that provider");

  assert_eq!(refused.key, "appMcp.errors.connectionRunsNoAgents");
  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
  assert!(!sheet.draft, "the draft the save made was discarded");
  assert_eq!(sheet.current, Some(first.id()), "nothing changed");
}
