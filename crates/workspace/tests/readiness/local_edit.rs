//! The select rows of a local profile's panel (`Agents · Profile` L1, ORC-842): the CLI, the model, the reasoning
//! effort and the MCP servers are changed one at a time into the profile's draft by the editor's own rules, and
//! publishing makes the draft the next version.

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  id::{ConnectionId, ResourceId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::{AgentProfile, Draft, ModelId, PromptText, ProviderSettings, RepositoryAccess},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_workspace::agents::{
  editor::Outcome,
  local_edit::{Edit, discard, edit, publish},
  local_hub::LocalHub,
};

use crate::support::{Fixture, name};

fn hub(fixture: &Fixture) -> LocalHub {
  fixture
    .workspace
    .local_hub(&fixture.root, &fixture.vault)
    .expect("the hub")
}

/// A connection of `provider` whose last check found `models`, each with its efforts.
fn checked(fixture: &Fixture, provider: AgentProvider, label: &str, models: &[(&str, &[&str])]) -> ConnectionId {
  let mut made = AgentConnection::new(
    ConnectionId::generate(),
    name(label),
    provider,
    fixture.root.join(format!("{label}.exe")),
    Timestamp::now(),
  )
  .expect("connection");
  let catalog = models
    .iter()
    .map(|(id, efforts)| {
      CatalogModel::new(
        ModelId::new(id).expect("model"),
        efforts.iter().map(|effort| (*effort).to_owned()).collect(),
      )
    })
    .collect();
  made
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "1.0.0".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, catalog),
      },
    })
    .expect("check");
  fixture.db().create_connection(&made).expect("a connection");
  made.id()
}

fn builder_on(connection: ConnectionId, model: &str) -> AgentProfile {
  telling(connection, model, "Build.")
}

/// Builder that tells its agents `instructions`.
fn telling(connection: ConnectionId, model: &str, instructions: &str) -> AgentProfile {
  AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new(instructions).expect("prompt"),
    connection,
    ModelId::new(model).expect("model"),
    ProviderSettings::default(),
  )
  // What every CLI enforces on every platform, so nothing is refused for its permissions or widens.
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AskBefore,
    NetworkAccess::Off,
  ))
}

/// Builder on a Codex connection that lists two models, published as v1; and a Claude Code connection.
fn project() -> (Fixture, ResourceId, [ConnectionId; 2]) {
  let fixture = Fixture::local();
  let codex = checked(
    &fixture,
    AgentProvider::CodexCli,
    "codex-main",
    &[("gpt-a", &["low", "high"]), ("gpt-b", &[])],
  );
  let claude = checked(
    &fixture,
    AgentProvider::ClaudeCodeCli,
    "claude-2",
    &[("claude-opus-5-5", &["low", "high"])],
  );
  let builder = fixture.own("builder", &builder_on(codex, "gpt-a"));
  (fixture, builder, [codex, claude])
}

#[test]
fn readiness_local_edit_a_field_is_a_draft_and_publishing_makes_the_next_version() {
  let (fixture, builder, _) = project();

  let saved = edit(&fixture.root, builder, &Edit::Effort(Some("high".into())), &[]).expect("saved");
  assert_eq!(saved, Outcome::Saved(builder));

  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
  assert!(sheet.draft, "the change is a draft, not a version");
  assert!(sheet.draft_saved_at.is_some());
  assert_eq!(
    sheet.effort.as_deref(),
    Some("high"),
    "the panel shows the draft's value"
  );
  assert_eq!(sheet.options.efforts, ["low", "high"]);
  assert_eq!(sheet.options.models, ["gpt-a", "gpt-b"]);

  assert_eq!(
    publish(&fixture.root, builder, shown(&fixture, builder), &[]).expect("published"),
    Outcome::Saved(builder)
  );
  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
  assert!(!sheet.draft);
  assert_eq!(sheet.effort.as_deref(), Some("high"));
  let found = read
    .readiness()
    .resources()
    .iter()
    .find(|found| found.resource() == builder)
    .expect("listed");
  assert_eq!(found.number().get(), 2);
}

#[test]
fn readiness_local_edit_a_model_resets_the_effort_and_one_the_connection_lacks_is_refused() {
  let (fixture, builder, _) = project();
  edit(&fixture.root, builder, &Edit::Effort(Some("low".into())), &[]).expect("effort");

  edit(&fixture.root, builder, &Edit::Model("gpt-b".to_owned()), &[]).expect("model");
  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
  assert_eq!(sheet.model, "gpt-b");
  assert_eq!(sheet.effort, None, "a new model starts at the CLI's default effort");

  let refused = edit(&fixture.root, builder, &Edit::Model("nope".to_owned()), &[]).expect_err("not offered");
  assert_eq!(refused.key, "agents.localEdit.modelNotOffered");
  let refused = edit(&fixture.root, builder, &Edit::Effort(Some("max".into())), &[]).expect_err("not offered");
  assert_eq!(refused.key, "agents.localEdit.effortNotOffered");
  assert_eq!(
    hub(&fixture).profile(builder).expect("a sheet").model,
    "gpt-b",
    "nothing else changed"
  );
}

#[test]
fn readiness_local_edit_another_cli_runs_on_its_connection_and_first_model() {
  let (fixture, builder, [_, claude]) = project();

  // What Claude Code can enforce differs from Codex's: the switch widens the network, so it asks first.
  let Outcome::Confirm(asked) =
    edit(&fixture.root, builder, &Edit::Cli(AgentProvider::ClaudeCodeCli), &[]).expect("asks")
  else {
    panic!("another CLI widens what the agents may do");
  };
  assert_eq!(
    hub(&fixture).profile(builder).expect("a sheet").cli,
    Some(AgentProvider::CodexCli),
    "nothing saved"
  );
  edit(&fixture.root, builder, &Edit::Cli(AgentProvider::ClaudeCodeCli), &asked).expect("allowed");

  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
  assert_eq!(sheet.cli, Some(AgentProvider::ClaudeCodeCli));
  assert_eq!(sheet.current, Some(claude));
  assert_eq!(sheet.model, "claude-opus-5-5");
  assert!(
    sheet.options.clis.contains(&AgentProvider::CodexCli) && sheet.options.clis.contains(&AgentProvider::ClaudeCodeCli)
  );
  let refused = edit(&fixture.root, builder, &Edit::Cli(AgentProvider::OpenAiApi), &[]).expect_err("no connection");
  assert_eq!(refused.key, "agents.localEdit.noConnection");
}

#[test]
fn readiness_local_edit_choosing_a_server_asks_first_and_an_archived_one_cannot_be_chosen() {
  let (fixture, builder, _) = project();
  let docs = fixture.local_server("docs", "docs-mcp");
  let old = fixture.local_server("old", "old-mcp");
  fixture
    .db()
    .archive_mcp_server(old, Timestamp::now())
    .expect("archived");
  let read = hub(&fixture);
  assert_eq!(
    read.profile(builder).expect("a sheet").options.servers,
    [(docs, "docs".to_owned())],
    "an archived server is not offered"
  );

  let Outcome::Confirm(asked) = edit(&fixture.root, builder, &Edit::AddServer(docs), &[]).expect("asks") else {
    panic!("choosing a server widens what its agents may do");
  };
  assert!(
    !hub(&fixture).profile(builder).expect("a sheet").draft,
    "nothing was saved while it asks"
  );

  edit(&fixture.root, builder, &Edit::AddServer(docs), &asked).expect("allowed");
  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
  assert_eq!(sheet.mcp_servers, [(docs, "docs".to_owned())]);
  assert!(sheet.options.servers.is_empty());

  edit(&fixture.root, builder, &Edit::RemoveServer(docs), &[]).expect("a removal widens nothing");
  assert!(hub(&fixture).profile(builder).expect("a sheet").mcp_servers.is_empty());

  let refused = edit(&fixture.root, builder, &Edit::AddServer(old), &asked).expect_err("archived");
  assert_eq!(refused.key, "appMcp.errors.mcpServerMissing");
}

/// When the draft the panel would show now was saved.
fn shown(fixture: &Fixture, profile: ResourceId) -> Timestamp {
  hub(fixture)
    .profile(profile)
    .and_then(|sheet| sheet.draft_saved_at)
    .expect("a draft")
}

#[test]
fn readiness_local_edit_a_profile_without_a_draft_has_nothing_to_publish() {
  let (fixture, builder, _) = project();

  let none = publish(&fixture.root, builder, Timestamp::now(), &[]).expect_err("no draft");
  assert_eq!(none.key, "appMcp.errors.noDraft");

  edit(&fixture.root, builder, &Edit::Effort(Some("low".into())), &[]).expect("a draft");
  let at = shown(&fixture, builder);
  assert_eq!(
    publish(&fixture.root, builder, at, &[]).expect("published"),
    Outcome::Saved(builder)
  );
  let again = publish(&fixture.root, builder, at, &[]).expect_err("published already");
  assert_eq!(again.key, "appMcp.errors.noDraft", "the draft is the version now");
}

#[test]
fn readiness_local_edit_publish_and_discard_act_on_the_draft_the_panel_showed() {
  let (fixture, builder, _) = project();
  edit(&fixture.root, builder, &Edit::Effort(Some("low".into())), &[]).expect("a draft");
  let showed = shown(&fixture, builder);

  // The draft is saved again, here or in the profile's editor, after the panel showed it.
  std::thread::sleep(std::time::Duration::from_millis(5));
  edit(&fixture.root, builder, &Edit::Effort(Some("high".into())), &[]).expect("another draft");
  assert_ne!(shown(&fixture, builder), showed, "the draft moved");

  let refused = publish(&fixture.root, builder, showed, &[]).expect_err("not the draft shown");
  assert_eq!(refused.key, "errors.draftChanged");
  let refused = discard(&fixture.root, builder, showed).expect_err("not the draft shown");
  assert_eq!(refused.key, "errors.draftChanged");
  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
  assert!(sheet.draft, "nothing was published or lost");
  assert_eq!(sheet.effort.as_deref(), Some("high"));

  let number = discard(&fixture.root, builder, shown(&fixture, builder)).expect("the shown draft goes");
  assert_eq!(number.get(), 1, "the published version is the profile again");
  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
  assert!(!sheet.draft);
  assert_eq!(sheet.effort, None);
}

#[test]
fn readiness_local_edit_a_draft_that_changes_the_prompts_is_published_in_the_editor() {
  let (fixture, builder, [codex, _]) = project();
  let latest = fixture
    .db()
    .latest_version::<AgentProfile>(builder)
    .expect("read")
    .expect("published");
  let body = telling(codex, "gpt-a", "Build, and also review everything.");
  fixture
    .db()
    .save_draft(builder, &Draft::new(body, Some(latest.number()), Timestamp::now()))
    .expect("a draft that changes what the role tells its agents");

  let refused = publish(&fixture.root, builder, shown(&fixture, builder), &[]).expect_err("the panel shows no prompt");
  assert_eq!(refused.key, "agents.localEdit.promptsInEditor");
  assert!(
    hub(&fixture).profile(builder).expect("a sheet").draft,
    "the draft is kept for the editor"
  );

  // A change of a field the panel draws, on top of that draft, does not make the prompts the panel's to publish.
  edit(&fixture.root, builder, &Edit::Effort(Some("low".into())), &[]).expect("saved into the draft");
  let refused = publish(&fixture.root, builder, shown(&fixture, builder), &[]).expect_err("still refused");
  assert_eq!(refused.key, "agents.localEdit.promptsInEditor");
}
