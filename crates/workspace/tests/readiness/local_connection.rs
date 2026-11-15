//! What a local profile runs on on this computer (`Agents on accounts` C1 `RwzH2`, `docs/design/agent-accounts.md`,
//! "Agents run on accounts", decision 4): a profile whose connection is gone names the CLI its model belongs to and
//! everything it could run on; choosing a connection of its CLI that serves its model applies at once with no new
//! version, and a *Save* of the panel publishes what it ran on and drops that choice.

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, ApiEndpoint, BaseUrl, CatalogModel, CatalogSource, CheckResult, ConnectionAccount,
    ConnectionCheck, EndpointPreset, ModelCatalog, SignIn,
  },
  id::{AgentAccountId, ConnectionId, ResourceId, SecretId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::{AgentProfile, Draft, ModelId, PromptText, ProviderSettings, RepositoryAccess, VersionNumber},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_workspace::{
  agents::{
    editor::Outcome,
    local_connection::run_here,
    local_edit::{self, Edit},
    local_hub::LocalHub,
    role::RoleText,
  },
  messages::Message,
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

/// The connection Builder runs on here, and the number of its latest version.
fn runs_on(fixture: &Fixture, builder: ResourceId) -> (ConnectionId, VersionNumber) {
  let db = fixture.db();
  let version = db
    .latest_version::<AgentProfile>(builder)
    .expect("read")
    .expect("published");
  let here = db.connection_here(builder, version.body().connection()).expect("read");
  (here, version.number())
}

#[test]
fn readiness_local_connection_choosing_one_runs_it_there_at_once_with_no_new_version() {
  let (fixture, builder, [claude_2, ..]) = gone();

  run_here(&fixture.root, builder, claude_2).expect("chosen");

  assert_eq!(runs_on(&fixture, builder), (claude_2, VersionNumber::FIRST));
  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
  assert_eq!(sheet.current, Some(claude_2));
  assert_eq!(sheet.connection.as_deref(), Some("claude-2"));
  assert!(
    fixture.db().draft::<AgentProfile>(builder).expect("read").is_none(),
    "no draft is made"
  );
  let found = read
    .readiness()
    .resources()
    .iter()
    .find(|found| found.resource() == builder)
    .expect("listed");
  assert_eq!(found.number(), VersionNumber::FIRST);
  assert_eq!(found.group(), Group::Ready);
}

#[test]
fn readiness_local_connection_refuses_another_cli_and_changes_nothing_while_a_draft_does_not_matter() {
  let (fixture, builder, [claude_2, _, codex]) = gone();

  let other = run_here(&fixture.root, builder, codex).expect_err("a Codex connection does not run Claude Code");
  assert_eq!(other.key, "agents.localConnection.notOfKind");
  assert_eq!(hub(&fixture).profile(builder).expect("a sheet").current, None);

  let draft = builder_on(ConnectionId::generate());
  fixture
    .db()
    .save_draft(builder, &Draft::new(draft.clone(), Some(VersionNumber::FIRST), at(20)))
    .expect("a draft");
  run_here(&fixture.root, builder, claude_2).expect("the choice is this computer's, not the draft's");

  assert_eq!(runs_on(&fixture, builder), (claude_2, VersionNumber::FIRST));
  let kept = fixture
    .db()
    .draft::<AgentProfile>(builder)
    .expect("read")
    .expect("kept");
  assert_eq!(kept.body(), &draft, "the draft is as it was");
}

/// A Claude Code connection whose last check listed `models`.
fn listing(fixture: &Fixture, label: &str, models: &[&str]) -> ConnectionId {
  let mut made = AgentConnection::new(
    ConnectionId::generate(),
    name(label),
    CLAUDE,
    fixture.root.join(format!("{label}.exe")),
    Timestamp::now(),
  )
  .expect("connection");
  let listed = models
    .iter()
    .map(|id| CatalogModel::new(ModelId::new(id).expect("model"), Vec::new()))
    .collect();
  made
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "1.0.0".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, listed),
      },
    })
    .expect("its kind");
  fixture.db().create_connection(&made).expect("a connection");
  made.id()
}

#[test]
fn readiness_local_connection_one_that_lists_other_models_is_not_taken() {
  let (fixture, builder, _) = gone();
  let other_models = listing(&fixture, "gateway", &["claude-sonnet-5-5"]);
  let same_model = listing(&fixture, "work", &["claude-opus-5-5", "claude-sonnet-5-5"]);

  let refused = run_here(&fixture.root, builder, other_models).expect_err("the model is the version's to name");
  assert_eq!(refused.key, "agents.localEdit.modelNotOffered");
  assert_eq!(hub(&fixture).profile(builder).expect("a sheet").current, None);

  run_here(&fixture.root, builder, same_model).expect("it lists the profile's model");
  assert_eq!(runs_on(&fixture, builder).0, same_model);
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
fn readiness_local_connection_work_moves_only_to_another_account_or_host_of_a_profile_that_ran_somewhere() {
  let fixture = Fixture::local();
  let home = fixture.connection(CLAUDE, "home");
  let work = on_account(&fixture, "work");
  let builder = fixture.own("builder", &builder_on(home));
  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
  let choice = |id| sheet.runs_on.iter().find(|choice| choice.id == id).expect("offered");
  let (home, work) = (choice(home), choice(work));

  assert!(work.moves_work(Some(home)), "another account of the person's");
  assert!(!home.moves_work(Some(work)), "back on the CLI's Default");
  assert!(!work.moves_work(Some(work)), "the same account");
  assert!(
    !work.moves_work(None),
    "nothing moves away from a profile that ran on nothing"
  );
}

#[test]
fn readiness_local_connection_a_save_publishes_what_it_ran_on_and_drops_this_computers_choice() {
  let fixture = Fixture::local();
  let home = fixture.connection(CLAUDE, "home");
  let work = on_account(&fixture, "work");
  let builder = fixture.own("builder", &builder_on(home));
  run_here(&fixture.root, builder, work).expect("chosen");
  assert_eq!(runs_on(&fixture, builder), (work, VersionNumber::FIRST));

  let role = RoleText {
    role: "Builder".into(),
    responsibilities: "Builds what the plan says.".into(),
    instructions: "Build.".into(),
  };
  let held = local_edit::hold(&fixture.root, builder, None, &Edit::Role(role)).expect("held");
  assert_eq!(
    held.body.connection(),
    work,
    "the panel's first change starts from what it runs on here"
  );
  let allowed = [Message::new("appMcp.widens.account")];
  let saved = local_edit::save(&fixture.root, builder, &held.body, (&allowed, true)).expect("saved");

  assert!(matches!(saved, Outcome::Saved(_)), "{saved:?}");
  let db = fixture.db();
  assert_eq!(db.connection_pick(builder).expect("read"), None);
  let version = db
    .latest_version::<AgentProfile>(builder)
    .expect("read")
    .expect("published");
  assert_eq!(version.number(), VersionNumber::FIRST.next().expect("v2"));
  assert_eq!(version.body().connection(), work, "v2 names what the panel showed");
}

#[test]
fn readiness_local_connection_one_that_runs_no_agents_is_not_taken() {
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

  let refused = run_here(&fixture.root, builder, second.id()).expect_err("this version has no client for it");

  assert_eq!(refused.key, "appMcp.errors.connectionRunsNoAgents");
  assert_eq!(runs_on(&fixture, builder).0, first.id(), "nothing changed");
}
