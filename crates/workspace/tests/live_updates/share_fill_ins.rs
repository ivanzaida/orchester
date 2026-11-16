//! What the sharer's computer fills in when it shares a local project's configuration, and what Share says it could not
//! carry (`docs/design/team-configuration.md`, edge case 12 and "What a computer fills in"), against the fake team
//! server: after Share the sharer's team MCP servers are Ready here, and everything Share did not carry, or carried
//! under another form, comes back in the result.
//!
//! `cargo test -p orchester-workspace --test live_updates live_updates_share_fill_ins_`

use std::{
  collections::BTreeMap,
  path::{Path, PathBuf},
};

use orchester_core::{
  connection::mcp::{McpAuthorization, McpEnvValue, McpEnvironment, McpServer, McpTransport, StdioServer},
  id::{ResourceId, ServerId},
  resource::{AgentProfile, CommandProgram, TeamMcpServer, VariableName},
  secret::{Secret, SecretName, SecretValue, SecretVault},
  sharing::{Here, McpNeed, Missing, Needs, ProgramProblem, Readiness, readiness},
  store::{McpServerStore, ResourceStore},
};
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  sharing::{
    NotSet, ShareNote, ShareRefusalReason, SharingError, TeamMcpProblem,
    sync::{FillInOutcome, ShareFillIns, upload_configuration},
  },
  workspace::Workspace,
};
use tempfile::TempDir;

use super::{
  ALICE, TeamServer,
  publish::Publisher,
  share::Sharer,
  share_team_form::{
    MemoryVault, PROGRAM, TOKEN, local_server, notes, profile_named, remote, reviewer, secret, workspace_with_server,
  },
};

/// A program file in a folder of its own, and the search path that holds that folder.
struct Installed {
  _folder: TempDir,
  file: PathBuf,
}

fn installed(program: &str) -> Installed {
  let folder = tempfile::tempdir().expect("a folder");
  let file = folder.path().join(if cfg!(windows) {
    format!("{program}.exe")
  } else {
    program.to_owned()
  });
  std::fs::write(&file, "#!/bin/sh\n").expect("a program file");
  Installed { _folder: folder, file }
}

fn stdio(label: &str, program: &str, variables: &[(&str, McpEnvValue)]) -> McpServer {
  let variables: BTreeMap<_, _> = variables
    .iter()
    .map(|(name, value)| (VariableName::new(name).expect("variable"), value.clone()))
    .collect();
  local_server(
    label,
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new(program).expect("program"),
      Vec::new(),
      None,
    )),
    McpEnvironment::new(variables).expect("variables"),
  )
}

fn resource_of(server: &McpServer) -> ResourceId {
  ResourceId::from_uuid(server.id().as_uuid())
}

fn store_value(vault: &MemoryVault, alice: &Sharer, secret: &Secret, value: &str) {
  vault
    .store(
      &secret.vault_key(alice.db().project().id()),
      &SecretValue::new(value.to_owned()).expect("a value"),
    )
    .expect("the local value");
}

/// The shared project's readiness for `resource` on this computer, from its latest version on `team` and what this
/// computer filled in.
fn readiness_of(
  (db, team): (&ProjectDb, &TeamServer),
  vault: &MemoryVault,
  (app, server): (&AppDb, ServerId),
  resource: ResourceId,
) -> Readiness {
  let latest = team.versions_of(resource).pop().expect("the team server is uploaded");
  let body: TeamMcpServer = serde_json::from_value(latest.body).expect("a team MCP server");
  let mut here = Here::nothing();
  if db.team_mcp_switch(resource).expect("read") == Some(true) {
    here = here.with_mcp_on(resource);
  }
  if let Some(program) = app.mcp_server_program(server, resource).expect("read") {
    let exists = program.program.exists();
    here = here.with_program(resource, program.program, exists);
  }
  for name in body.secrets() {
    if db.team_secret_value(vault, &name).expect("read").is_some() {
      here = here.with_secret(name);
    }
  }
  readiness(&Needs::mcp_server(&McpNeed::of(resource, &body)), &here)
}

/// Reads the fill-ins, shares, and applies them as Share does.
async fn share_and_fill(
  alice: &Sharer,
  team: &TeamServer,
  vault: &MemoryVault,
  search_path: Option<&Path>,
) -> (FillInOutcome, (Workspace, (TempDir, PathBuf), ServerId)) {
  let root = alice.fixture.repo.path().to_owned();
  let (workspace, folder, server) = workspace_with_server();
  let search_path = search_path.map(Path::as_os_str);
  let fill_ins = ShareFillIns::read(&root, vault, search_path).expect("read before the upload");
  alice.share(team).await;
  let outcome = fill_ins.apply((&workspace, vault), &root, server);
  (outcome, (workspace, folder, server))
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_fill_ins_leave_the_sharer_ready_with_a_path_and_with_a_name() {
  let team = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut alice = Sharer::new();
  let token = secret(&mut alice.fixture.db, "NOTES_TOKEN");
  let absolute = installed("notes-mcp");
  let by_name = installed("npx");
  let with_path = notes(&token, absolute.file.to_str().expect("text"));
  let with_name = stdio("runner", "npx", &[]);
  for kept in [&with_path, &with_name] {
    alice.fixture.db.create_mcp_server(kept).expect("a local MCP server");
  }
  let reviewer = reviewer(&mut alice, &[with_path.id(), with_name.id()]);
  let vault = MemoryVault::default();
  store_value(&vault, &alice, &token, TOKEN);
  let (_, builder) = alice.delivery();
  let connection = alice
    .db()
    .latest_version::<AgentProfile>(builder)
    .expect("read")
    .expect("Builder")
    .body()
    .connection();

  let (outcome, (_workspace, (_folder, app_db), server)) =
    share_and_fill(&alice, &team, &vault, Some(by_name.file.parent().expect("a folder"))).await;

  assert!(outcome.not_set.is_empty(), "{:?}", outcome.not_set);
  let (db, app) = (alice.db(), AppDb::open(&app_db).expect("application database"));
  for server_here in [&with_path, &with_name] {
    let ready = readiness_of((&db, &team), &vault, (&app, server), resource_of(server_here));
    assert!(ready.is_ready(), "{:?}", ready.missing());
  }
  let path = app.mcp_server_program(server, resource_of(&with_name)).expect("read");
  assert_eq!(path.expect("a path").program, by_name.file, "the name, looked up here");
  let path = app.mcp_server_program(server, resource_of(&with_path)).expect("read");
  assert_eq!(path.expect("a path").program, absolute.file);
  assert_eq!(
    db.connection_pick(reviewer).expect("read").expect("a pick").connection,
    connection
  );
  assert!(db.connection_pick(builder).expect("read").is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_fill_ins_store_every_value_under_the_team_secret_that_takes_it() {
  let team = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut alice = Sharer::new();
  let token = secret(&mut alice.fixture.db, "NOTES_TOKEN");
  let installed = installed("notes-mcp");
  let program = installed.file.to_str().expect("text");
  let taking = [
    ("API_TOKEN", McpEnvValue::Secret(token.id())),
    ("OTHER_TOKEN", McpEnvValue::Secret(token.id())),
    ("MODE", McpEnvValue::Text("fast".to_owned())),
  ];
  let local = stdio("notes", program, &taking);
  alice.fixture.db.create_mcp_server(&local).expect("a local MCP server");
  reviewer(&mut alice, &[local.id()]);
  let vault = MemoryVault::default();
  store_value(&vault, &alice, &token, TOKEN);

  let (outcome, _) = share_and_fill(&alice, &team, &vault, None).await;

  let db = alice.db();
  let value = |name: &str| {
    let name = SecretName::new(name).expect("name");
    db.team_secret_value(&vault, &name)
      .expect("read")
      .map(|value| value.expose().to_owned())
  };
  assert_eq!(
    value("API_TOKEN").as_deref(),
    Some(TOKEN),
    "a variable takes the secret of its own name"
  );
  assert_eq!(
    value("OTHER_TOKEN").as_deref(),
    Some(TOKEN),
    "one local secret, two variables"
  );
  assert_eq!(
    value("MODE").as_deref(),
    Some("fast"),
    "a plain value is a team secret's value here"
  );
  assert_eq!(value("NOTES_TOKEN"), None, "the local secret's name is not the team's");
  assert!(outcome.not_set.is_empty(), "{:?}", outcome.not_set);
  assert_eq!(
    outcome.notes,
    [ShareNote::VariablesAreSecrets {
      server: "notes".to_owned(),
      variables: vec!["MODE".to_owned()],
    }]
  );
  assert!(
    !format!("{outcome:?}").contains("fast"),
    "the value is not in the result"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_fill_ins_carry_a_header_secret_by_its_form() {
  let team = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut alice = Sharer::new();
  let key = secret(&mut alice.fixture.db, "ISSUES_KEY");
  let basic = secret(&mut alice.fixture.db, "WIKI_BASIC");
  let header = remote(
    "issues",
    "https://mcp.example.dev/issues",
    McpAuthorization::Header {
      name: orchester_core::connection::mcp::McpHeaderName::new("X-Api-Key").expect("header"),
      value: key.id(),
    },
  );
  let wiki = remote(
    "wiki",
    "https://mcp.example.dev/wiki",
    McpAuthorization::BasicCredential { credential: basic.id() },
  );
  for kept in [&header, &wiki] {
    alice.fixture.db.create_mcp_server(kept).expect("a local MCP server");
  }
  reviewer(&mut alice, &[header.id(), wiki.id()]);
  let vault = MemoryVault::default();
  store_value(&vault, &alice, &key, "key-value-1");
  store_value(&vault, &alice, &basic, "Basic dXNlcjpwYXNz");

  let (outcome, _) = share_and_fill(&alice, &team, &vault, None).await;

  let db = alice.db();
  let team_body = |server: &McpServer| {
    let sent = team.versions_of(resource_of(server));
    serde_json::from_value::<TeamMcpServer>(sent[0].body.clone()).expect("a team MCP server")
  };
  let headers = |body: &TeamMcpServer| match body.launch() {
    orchester_core::resource::TeamMcpLaunch::Remote(remote) => remote
      .secret_headers()
      .iter()
      .map(|found| {
        (
          found.header().as_str().to_owned(),
          found.secret().as_str().to_owned(),
          found.prefix().map(|prefix| prefix.as_str().to_owned()),
        )
      })
      .collect::<Vec<_>>(),
    other => panic!("{other:?}"),
  };
  assert_eq!(
    headers(&team_body(&header)),
    [("X-Api-Key".to_owned(), "ISSUES_KEY".to_owned(), None)]
  );
  assert_eq!(
    headers(&team_body(&wiki)),
    [(
      "Authorization".to_owned(),
      "WIKI_BASIC".to_owned(),
      Some("Basic ".to_owned())
    )]
  );
  let value = |name: &str| {
    db.team_secret_value(&vault, &SecretName::new(name).expect("name"))
      .expect("read")
      .map(|value| value.expose().to_owned())
  };
  assert_eq!(value("ISSUES_KEY").as_deref(), Some("key-value-1"));
  assert_eq!(
    value("WIKI_BASIC").as_deref(),
    Some("dXNlcjpwYXNz"),
    "pasted with the scheme, the team holds the part after the prefix"
  );
  assert!(outcome.not_set.is_empty(), "{:?}", outcome.not_set);
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_fill_ins_say_the_search_path_name_they_could_not_find() {
  let team = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut alice = Sharer::new();
  let with_name = stdio("runner", "npx", &[]);
  alice
    .fixture
    .db
    .create_mcp_server(&with_name)
    .expect("a local MCP server");
  reviewer(&mut alice, &[with_name.id()]);
  let nowhere = installed("something-else");
  let vault = MemoryVault::default();

  let (outcome, (_workspace, (_folder, app_db), server)) =
    share_and_fill(&alice, &team, &vault, Some(nowhere.file.parent().expect("a folder"))).await;

  assert_eq!(
    outcome.not_set,
    [NotSet::McpProgramPath {
      server: "runner".to_owned()
    }]
  );
  let (db, app) = (alice.db(), AppDb::open(&app_db).expect("application database"));
  let ready = readiness_of((&db, &team), &vault, (&app, server), resource_of(&with_name));
  assert_eq!(
    ready.missing(),
    [Missing::McpProgram {
      server: resource_of(&with_name),
      problem: ProgramProblem::NotSet
    }],
    "the server is on, and only its path is left to set"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_fill_ins_apply_every_one_and_say_what_failed() {
  let team = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut alice = Sharer::new();
  let token = secret(&mut alice.fixture.db, "NOTES_TOKEN");
  let lost = secret(&mut alice.fixture.db, "LOST_TOKEN");
  let first = installed("notes-mcp");
  let second = installed("other-mcp");
  let with_secrets = stdio(
    "notes",
    first.file.to_str().expect("text"),
    &[
      ("API_TOKEN", McpEnvValue::Secret(token.id())),
      ("LOST", McpEnvValue::Secret(lost.id())),
    ],
  );
  let after = stdio("other", second.file.to_str().expect("text"), &[]);
  for kept in [&with_secrets, &after] {
    alice.fixture.db.create_mcp_server(kept).expect("a local MCP server");
  }
  let reviewer = reviewer(&mut alice, &[with_secrets.id(), after.id()]);
  let vault = MemoryVault::default();
  store_value(&vault, &alice, &token, TOKEN);
  let root = alice.fixture.repo.path().to_owned();
  let (workspace, (_folder, app_db), server) = workspace_with_server();
  let fill_ins = ShareFillIns::read(&root, &vault, None).expect("read before the upload");
  alice.share(&team).await;
  vault.refuse_stores();

  let outcome = fill_ins.apply((&workspace, &vault), &root, server);

  let named = |name: &str| SecretName::new(name).expect("name");
  assert_eq!(
    outcome.not_set,
    [
      NotSet::SecretValue { secret: named("LOST") },
      NotSet::SecretKept {
        secret: named("API_TOKEN")
      },
    ],
    "the missing value is said when it is read, the refused store when it is kept"
  );
  let (db, app) = (alice.db(), AppDb::open(&app_db).expect("application database"));
  assert!(
    db.team_mcp_switch(resource_of(&after)).expect("read") == Some(true),
    "later fill-ins are still set"
  );
  assert!(
    app
      .mcp_server_program(server, resource_of(&after))
      .expect("read")
      .is_some()
  );
  assert!(db.connection_pick(reviewer).expect("read").is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_refuses_a_variable_that_is_not_a_secret_name_and_two_servers_filling_one_team_secret_differently()
 {
  let team = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut alice = Sharer::new();
  let token = secret(&mut alice.fixture.db, "NOTES_TOKEN");
  let other = secret(&mut alice.fixture.db, "OTHER_TOKEN");
  let lower = stdio("lower", PROGRAM, &[("mode", McpEnvValue::Text("fast".to_owned()))]);
  let first = stdio("first", PROGRAM, &[("API_TOKEN", McpEnvValue::Secret(token.id()))]);
  let second = stdio("second", PROGRAM, &[("API_TOKEN", McpEnvValue::Secret(other.id()))]);
  for kept in [&lower, &first, &second] {
    alice.fixture.db.create_mcp_server(kept).expect("a local MCP server");
  }
  reviewer(&mut alice, &[lower.id(), first.id(), second.id()]);
  let publisher = Publisher::online(&team, ALICE);

  let refused = upload_configuration(&publisher, alice.fixture.repo.path()).await;

  let Err(SharingError::ShareRefused { refusals }) = &refused else {
    panic!("{refused:?}");
  };
  let problems: Vec<_> = refusals
    .iter()
    .map(|refusal| match &refusal.reason {
      ShareRefusalReason::McpServer { server, problem } => (server.as_str(), problem.clone()),
      other => panic!("{other:?}"),
    })
    .collect();
  assert_eq!(
    problems,
    [
      (
        "lower",
        TeamMcpProblem::VariableNotSecret {
          variable: "mode".to_owned()
        }
      ),
      (
        "second",
        TeamMcpProblem::SecretTaken {
          secret: "API_TOKEN".to_owned(),
          other: "first".to_owned()
        }
      ),
    ]
  );
  let address = orchester_api::client::ServerAddress::parse("https://team.test").expect("an address");
  let said = refused.expect_err("refused").message(&address);
  assert_eq!(said.key, "sharing.errors.shareRefused");
  let names = &said
    .args
    .iter()
    .find(|(name, _)| *name == "resources")
    .expect("the resources")
    .1;
  assert_eq!(names, "lower, second", "the refused resources, by name");
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_says_an_older_version_loses_a_server_the_team_cannot_take_or_that_is_gone() {
  let team = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut alice = Sharer::new();
  let in_tree = stdio("scripts", "tools/serve.sh", &[]);
  let gone = orchester_core::id::ConnectionId::generate();
  alice
    .fixture
    .db
    .create_mcp_server(&in_tree)
    .expect("a local MCP server");
  let (_, builder) = alice.delivery();
  let base = alice
    .db()
    .latest_version::<AgentProfile>(builder)
    .expect("read")
    .expect("Builder")
    .body()
    .clone();
  let choosing = base.clone().with_mcp_servers(
    orchester_core::connection::mcp::McpServerSelection::new(vec![in_tree.id(), gone]).expect("one server"),
  );
  let history = profile_named(&mut alice, "history", &choosing);
  let db = &mut alice.fixture.db;
  db.save_draft(
    history,
    &orchester_core::resource::Draft::new(base, Some(super::publish::number(1)), super::support::at(40)),
  )
  .expect("a draft of v2");
  db.publish_draft::<AgentProfile>(history, super::support::at(41))
    .expect("v2");
  let vault = MemoryVault::default();
  let root = alice.fixture.repo.path().to_owned();

  let fill_ins = ShareFillIns::read(&root, &vault, None).expect("the Share is not refused for an old version");
  let publisher = Publisher::online(&team, ALICE);
  let uploaded = upload_configuration(&publisher, &root).await;

  assert!(uploaded.is_ok(), "{uploaded:?}");
  let outcome = fill_ins.apply((&workspace_with_server().0, &vault), &root, ServerId::generate());
  assert_eq!(
    outcome.notes,
    [
      ShareNote::UnsharedServerLeftOut {
        profile: "history".to_owned(),
        version: 1,
        server: "scripts".to_owned()
      },
      ShareNote::GoneServerLeftOut {
        profile: "history".to_owned(),
        version: 1
      },
    ]
  );
}
