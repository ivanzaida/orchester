//! The select rows of a local profile's panel (`Agents · Profile` L1, ORC-842): the CLI, the model, the reasoning
//! effort and the MCP servers are changed one at a time into the profile's draft by the editor's own rules, and
//! publishing makes the draft the next version. The hub shows the published version, never the draft.

use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  id::{ConnectionId, ResourceId},
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, AgentTargets, ApprovalMode, CommandAction, CommandProgram,
    DeniedBehavior, Draft, InjectionEvent, Instruction, InstructionScope, ModelId, PromptText, ProviderSettings,
    RepositoryAccess, Skill, Tool, ToolAction,
  },
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_workspace::agents::{
  editor::Outcome,
  form::stored_effort,
  local_edit::{Edit, GivenKind, edit, hold, publish, save},
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

  let draft = drafted(&fixture, builder).expect("the change is a draft, not a version");
  assert_eq!(stored_effort(&draft).as_deref(), Some("high"));
  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
  assert_eq!(
    sheet.effort, None,
    "the panel shows the published version, never a draft"
  );
  assert_eq!(sheet.options.efforts, ["low", "high"]);
  assert_eq!(sheet.options.models, ["gpt-a", "gpt-b"]);

  assert_eq!(
    publish(&fixture.root, builder, shown(&fixture, builder), &[]).expect("published"),
    Outcome::Saved(builder)
  );
  assert!(drafted(&fixture, builder).is_none());
  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
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
  let draft = drafted(&fixture, builder).expect("a draft");
  assert_eq!(draft.model().as_str(), "gpt-b");
  assert_eq!(
    stored_effort(&draft),
    None,
    "a new model starts at the CLI's default effort"
  );

  let refused = edit(&fixture.root, builder, &Edit::Model("nope".to_owned()), &[]).expect_err("not offered");
  assert_eq!(refused.key, "agents.localEdit.modelNotOffered");
  let refused = edit(&fixture.root, builder, &Edit::Effort(Some("max".into())), &[]).expect_err("not offered");
  assert_eq!(refused.key, "agents.localEdit.effortNotOffered");
  assert_eq!(
    drafted(&fixture, builder).expect("a draft").model().as_str(),
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
  assert!(drafted(&fixture, builder).is_none(), "nothing saved");
  edit(&fixture.root, builder, &Edit::Cli(AgentProvider::ClaudeCodeCli), &asked).expect("allowed");

  let draft = drafted(&fixture, builder).expect("a draft");
  assert_eq!(draft.connection(), claude);
  assert_eq!(draft.model().as_str(), "claude-opus-5-5");
  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
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
  assert!(drafted(&fixture, builder).is_none(), "nothing was saved while it asks");

  edit(&fixture.root, builder, &Edit::AddServer(docs), &asked).expect("allowed");
  publish(&fixture.root, builder, shown(&fixture, builder), &asked).expect("published");
  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
  assert_eq!(sheet.mcp_servers, [(docs, "docs".to_owned())]);
  assert!(sheet.options.servers.is_empty());

  edit(&fixture.root, builder, &Edit::RemoveServer(docs), &[]).expect("a removal widens nothing");
  let draft = drafted(&fixture, builder).expect("a draft");
  assert!(draft.mcp_servers().ids().is_empty());

  let refused = edit(&fixture.root, builder, &Edit::AddServer(old), &asked).expect_err("archived");
  assert_eq!(refused.key, "appMcp.errors.mcpServerMissing");
}

/// The stored draft of `profile`, which the hub's panel never shows; `None` without one.
fn drafted(fixture: &Fixture, profile: ResourceId) -> Option<AgentProfile> {
  fixture
    .db()
    .draft::<AgentProfile>(profile)
    .expect("read")
    .map(|draft| draft.body().clone())
}

/// When the stored draft of `profile` was saved, which a publish names.
fn shown(fixture: &Fixture, profile: ResourceId) -> Timestamp {
  fixture
    .db()
    .draft::<AgentProfile>(profile)
    .expect("read")
    .expect("a draft")
    .updated_at()
}

/// The hub's panel has no drafts (decided by the person 2026-11-09): what it holds starts from the published version
/// even when a draft is stored (an MCP client's), and its Save publishes what it holds over that draft.
#[test]
fn readiness_local_edit_the_panel_holds_the_published_version_and_its_save_replaces_a_draft() {
  let (fixture, builder, _) = project();
  edit(&fixture.root, builder, &Edit::Model("gpt-b".to_owned()), &[]).expect("a draft");

  let held = hold(&fixture.root, builder, None, &Edit::Effort(Some("low".into()))).expect("held");
  assert_eq!(
    held.body.model().as_str(),
    "gpt-a",
    "the draft's model is not the panel's"
  );
  assert_eq!(stored_effort(&held.body).as_deref(), Some("low"));
  assert!(held.changed);

  assert_eq!(
    save(&fixture.root, builder, &held.body, (&[], false)).expect("saved"),
    Outcome::Saved(builder)
  );
  assert!(drafted(&fixture, builder).is_none(), "the draft is replaced");
  let latest = fixture
    .db()
    .latest_version::<AgentProfile>(builder)
    .expect("read")
    .expect("published");
  assert_eq!(latest.number().get(), 2);
  assert_eq!(latest.body(), &held.body);
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
fn readiness_local_edit_publish_acts_on_the_draft_it_names() {
  let (fixture, builder, _) = project();
  edit(&fixture.root, builder, &Edit::Effort(Some("low".into())), &[]).expect("a draft");
  let showed = shown(&fixture, builder);

  // The draft is saved again, here or in the profile's editor, after the panel showed it.
  std::thread::sleep(std::time::Duration::from_millis(5));
  edit(&fixture.root, builder, &Edit::Effort(Some("high".into())), &[]).expect("another draft");
  assert_ne!(shown(&fixture, builder), showed, "the draft moved");

  let refused = publish(&fixture.root, builder, showed, &[]).expect_err("not the draft shown");
  assert_eq!(refused.key, "errors.draftChanged");
  let draft = drafted(&fixture, builder).expect("nothing was published or lost");
  assert_eq!(stored_effort(&draft).as_deref(), Some("high"));
  assert_eq!(hub(&fixture).profile(builder).expect("a sheet").effort, None);
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
  assert!(drafted(&fixture, builder).is_some(), "the draft is kept for the editor");

  // A change of a field the panel draws, on top of that draft, does not make the prompts the panel's to publish.
  edit(&fixture.root, builder, &Edit::Effort(Some("low".into())), &[]).expect("saved into the draft");
  let refused = publish(&fixture.root, builder, shown(&fixture, builder), &[]).expect_err("still refused");
  assert_eq!(refused.key, "agents.localEdit.promptsInEditor");
}

/// What a profile gives its agents (`docs/design/team-configuration.md`, "Decided 2026-11-09"): the project's
/// instructions, skills and tools, each chosen and taken back as an MCP server is, without asking first; an archived
/// one cannot be given, nor a tool no agent can call (a command without access rules).
#[test]
fn readiness_local_edit_gives_and_takes_back_instructions_skills_and_tools() {
  let (fixture, builder, _) = project();
  let rule = Instruction::new(
    PromptText::new("Be brief.").expect("prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("instruction");
  let brief = fixture.own("brief", &rule);
  let old = fixture.own("old", &rule);
  let skill = Skill::new("When.".into(), "How.".into(), Vec::new(), InstructionScope::Project).expect("skill");
  let ship = fixture.own("ship", &skill);
  let action = CommandAction::new(CommandProgram::new("cargo").expect("program"), Vec::new());
  let access = AccessPolicy::new(
    BTreeMap::new(),
    name("Local"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("access");
  let check = fixture.own(
    "check",
    &Tool::new("Checks.", ToolAction::Command(action.clone()), Some(access)).expect("a check"),
  );
  let plain = fixture.own("plain", &Tool::command("Runs.", action));
  fixture
    .db()
    .archive_local_resource(old, Timestamp::now())
    .expect("archived");
  let offered = hub(&fixture).profile(builder).expect("a sheet").options.given.clone();
  assert_eq!(
    offered.instructions,
    [(brief, "brief".to_owned())],
    "an archived one is not offered"
  );
  assert_eq!(
    offered.tools,
    [(check, "check".to_owned())],
    "a tool no agent can call is not offered"
  );

  for (kind, id) in [
    (GivenKind::Instruction, brief),
    (GivenKind::Skill, ship),
    (GivenKind::Tool, check),
  ] {
    let Outcome::Saved(_) = edit(&fixture.root, builder, &Edit::Give(kind, id), &[]).expect("given") else {
      panic!("giving {kind:?} asks nothing");
    };
  }
  publish(&fixture.root, builder, shown(&fixture, builder), &[]).expect("published");
  let read = hub(&fixture);
  let sheet = read.profile(builder).expect("a sheet");
  assert_eq!(sheet.given.instructions, [(brief, "brief".to_owned())]);
  assert_eq!(sheet.given.skills, [(ship, "ship".to_owned())]);
  assert_eq!(sheet.given.tools, [(check, "check".to_owned())]);
  assert!(sheet.options.given.instructions.is_empty());
  assert_eq!(read.profiles_giving(ship), ["builder"]);

  edit(&fixture.root, builder, &Edit::TakeBack(GivenKind::Skill, ship), &[]).expect("taken back");
  assert!(drafted(&fixture, builder).expect("a draft").skills().is_empty());

  let refused = edit(&fixture.root, builder, &Edit::Give(GivenKind::Instruction, old), &[]).expect_err("archived");
  assert_eq!(refused.key, "agents.localEdit.givenGone");
  let wrong = edit(&fixture.root, builder, &Edit::Give(GivenKind::Skill, brief), &[]).expect_err("not a skill");
  assert_eq!(wrong.key, "agents.localEdit.givenGone");
  let uncallable = edit(&fixture.root, builder, &Edit::Give(GivenKind::Tool, plain), &[]).expect_err("no check");
  assert_eq!(uncallable.key, "agents.localEdit.givenGone");
}
