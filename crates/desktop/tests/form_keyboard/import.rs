//! Import resources (06.04–06.06), opened from the sidebar over the page shown: Personal website's
//! Preview release and what it refers to are copied into Kontur as drafts, the profile's
//! connection mapped to Kontur's own. Personal website's database is only read.

use std::{
  collections::BTreeSet,
  fs,
  path::{Path, PathBuf},
  time::SystemTime,
};

use lurq::node::element::ElementRef;
use orchester_core::{
  connection::{
    AgentEndpoint, AgentProvider, Connection, ConnectionKind,
    mcp::{HttpServer, McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, McpUrl},
  },
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, AgentTargets, InjectionEvent, Instruction, InstructionScope, ModelId, Pipeline, PromptText,
    ProviderSettings, RepositoryAccess, ResourceKind, Slug, Step, StepKey, StepKind,
  },
  store::{ConnectionStore, McpServerStore, ResourceStore},
  time::Timestamp,
};
use orchester_desktop::activity::{Place, Target};
use orchester_store::ProjectDb;

use crate::harness::{Desktop, expected};

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn codex(db: &mut ProjectDb) -> ConnectionId {
  let connection = Connection::new(
    ConnectionId::generate(),
    name("Codex"),
    ConnectionKind::Agent(AgentProvider::CodexCli),
    AgentEndpoint::Program(std::env::temp_dir().join("codex")),
    Timestamp::now(),
  )
  .expect("absolute executable");
  db.create_connection(&connection).expect("stored");
  connection.id()
}

fn mcp(db: &mut ProjectDb, label: &str) -> ConnectionId {
  let server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new(label).expect("valid server name"),
    McpTransport::Http(HttpServer::new(
      McpUrl::new("https://mcp.example.test/v1").expect("valid URL"),
      None,
    )),
    McpEnvironment::default(),
    Timestamp::now(),
  )
  .expect("valid server");
  db.create_mcp_server(&server).expect("stored");
  server.id()
}

#[test]
fn import_profile_maps_its_mcp_server_in_the_dependency_dialog() {
  let mut source = None;
  let mut target_server = None;
  let mut desktop = Desktop::with_other_project(
    |repo| {
      let mut db = ProjectDb::open(repo).expect("source project");
      let connection = codex(&mut db);
      let server = mcp(&mut db, "source-docs");
      let profile = AgentProfile::new(
        name("Builder"),
        String::new(),
        PromptText::new("Build.").expect("prompt"),
        connection,
        ModelId::new("gpt-5.5").expect("model"),
        ProviderSettings::default(),
      )
      .with_mcp_servers(McpServerSelection::new(vec![server]).expect("one server"));
      let id = db
        .create_resource(
          Slug::new("builder").expect("slug"),
          name("Builder"),
          &profile,
          Timestamp::now(),
        )
        .expect("profile")
        .id();
      source = Some((id, server));
    },
    |repo| {
      let mut db = ProjectDb::open(repo).expect("target project");
      target_server = Some(mcp(&mut db, "target-docs"));
    },
  );
  let (profile, source_server) = source.expect("seeded profile");
  let target_server = target_server.expect("seeded local server");
  desktop.wait_for("nav-import");
  desktop.press("nav-import");
  desktop.wait_for("import-dialog");
  desktop.press("import-tab-agents");
  let resource = format!("import-resource-{profile}");
  desktop.wait_for(&resource);
  desktop.press(&resource);
  desktop.press("import-resolve");
  let row = format!("import-mapping-{source_server}");
  desktop.wait_for(&row);
  assert!(shows(&mut desktop, &row, "MCP server"));
  let status = format!("import-status-{source_server}");
  assert!(shows(&mut desktop, &status, "Unresolved"));
  choose(
    &mut desktop,
    &format!("import-choice-{source_server}"),
    "Use target-docs",
  );
  assert!(shows(&mut desktop, &status, "Mapped"));
  desktop.press("import-submit");
  desktop.wait_for("agent-editor");

  let db = ProjectDb::open(desktop.repo()).expect("target project");
  let copied = db.resources(ResourceKind::AgentProfile).expect("profiles");
  assert_eq!(copied.len(), 1);
  let draft = db
    .draft::<AgentProfile>(copied[0].id())
    .expect("read")
    .expect("imported draft");
  assert_eq!(draft.body().mcp_servers().ids(), [target_server]);
  assert_ne!(draft.body().mcp_servers().ids(), [source_server]);
}

/// What Personal website holds: Preview release v1 runs Builder, whose connection is Codex, and
/// has Release rules.
#[derive(Clone, Copy)]
struct Source {
  pipeline: ResourceId,
  connection: ConnectionId,
}

fn seed_source(repo: &Path) -> Source {
  let mut db = ProjectDb::open(repo).expect("open Personal website");
  let now = Timestamp::now();
  let connection = codex(&mut db);
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    connection,
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  );
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, now)
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(builder, now).expect("published");
  let step = Step {
    key: StepKey::new("build").expect("key"),
    name: name("Build"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline = db
    .create_resource(
      Slug::new("preview-release").expect("slug"),
      name("Preview release"),
      &Pipeline::new(vec![step]).expect("pipeline"),
      now,
    )
    .expect("pipeline")
    .id();
  db.publish_draft::<Pipeline>(pipeline, now).expect("published");
  let rules = Instruction::new(
    PromptText::new("Keep the changelog.").expect("prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Pipeline { pipeline },
  )
  .expect("instruction");
  db.create_resource(
    Slug::new("release-rules").expect("slug"),
    name("Release rules"),
    &rules,
    now,
  )
  .expect("instruction");
  Source { pipeline, connection }
}

fn database(repo: &Path) -> PathBuf {
  repo.join(".orchester").join("orchester.db")
}

fn fingerprint(repo: &Path) -> (Vec<u8>, SystemTime) {
  let path = database(repo);
  let modified = fs::metadata(&path)
    .and_then(|meta| meta.modified())
    .expect("modified time");
  (fs::read(&path).expect("the database file"), modified)
}

fn contains_text(element: ElementRef<'_>, text: &str) -> bool {
  element.text_content().is_some_and(|content| content.contains(text))
    || element.children().into_iter().any(|child| contains_text(child, text))
}

fn shows(desktop: &mut Desktop, id: &str, text: &str) -> bool {
  let element = desktop
    .tree
    .get_element_by_id(id)
    .unwrap_or_else(|| panic!("{id} is shown"));
  contains_text(element, text)
}

fn choose(desktop: &mut Desktop, id: &str, label: &str) {
  let mut select = desktop
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_select())
    .unwrap_or_else(|| panic!("the select {id}"));
  let index = select
    .labels()
    .iter()
    .position(|option| option.contains(label))
    .unwrap_or_else(|| panic!("{id} offers {label}"));
  select.commit(index);
  desktop.pass();
}

fn selected(desktop: &mut Desktop, id: &str) -> Vec<String> {
  desktop
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_select())
    .unwrap_or_else(|| panic!("the select {id}"))
    .selected_labels()
    .iter()
    .map(ToString::to_string)
    .collect()
}

fn type_name(desktop: &mut Desktop, id: &str, name: &str) {
  desktop
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_text_input())
    .expect("the name of the copy")
    .set_value(name);
  desktop.pass();
}

/// Whether `id` is drawn with the error stroke.
fn invalid(desktop: &mut Desktop, id: &str) -> bool {
  desktop
    .tree
    .get_element_by_id(id)
    .unwrap_or_else(|| panic!("{id} is shown"))
    .has_class("invalid")
}

/// Kontur with Personal website beside it, the import open from the sidebar with Preview release
/// selected and its dependencies shown.
fn resolving(kontur: impl FnOnce(&Path)) -> (Desktop, Source, ConnectionId) {
  let mut source = None;
  let mut own = None;
  let mut desktop = Desktop::with_other_project(
    |repo| source = Some(seed_source(repo)),
    |repo| {
      let mut db = ProjectDb::open(repo).expect("open Kontur");
      own = Some(codex(&mut db));
      drop(db);
      kontur(repo);
    },
  );
  let (source, own) = (source.expect("seeded"), own.expect("seeded"));
  desktop.wait_for("nav-import");
  desktop.press("nav-import");
  desktop.wait_for("import-dialog");
  let row = format!("import-resource-{}", source.pipeline);
  desktop.wait_for(&row);
  assert!(shows(&mut desktop, "import-destination", "Kontur"));
  desktop.press(&row);
  assert!(shows(&mut desktop, "import-selection", "1 selected"));
  assert!(shows(&mut desktop, "import-selection", "3 dependencies to resolve"));
  desktop.press("import-resolve");
  desktop.wait_for(&format!("import-mapping-{}", source.connection));
  (desktop, source, own)
}

fn resources(repo: &Path, kind: ResourceKind) -> Vec<(ResourceId, String)> {
  ProjectDb::open(repo)
    .expect("open Kontur")
    .resources(kind)
    .expect("listed")
    .into_iter()
    .map(|resource| (resource.id(), resource.name().as_str().to_owned()))
    .collect()
}

#[test]
fn import_copies_a_pipeline_with_what_it_needs_as_drafts() {
  let (mut desktop, source, own) = resolving(|_| {});
  let before = fingerprint(&desktop.other_repo());
  let connection = format!("import-status-{}", source.connection);
  assert!(shows(&mut desktop, &connection, "Unresolved"));

  choose(
    &mut desktop,
    &format!("import-choice-{}", source.connection),
    "Use Codex",
  );
  assert!(shows(&mut desktop, &connection, "Mapped"));
  assert!(shows(&mut desktop, "import-readiness", "All dependencies resolved"));
  assert!(shows(&mut desktop, "import-totals", "3 new copies"));
  desktop.press("import-submit");
  desktop.wait_for("pipeline-state");

  let repo = desktop.repo().to_path_buf();
  let pipelines = resources(&repo, ResourceKind::Pipeline);
  let profiles = resources(&repo, ResourceKind::AgentProfile);
  assert_eq!(
    pipelines.iter().map(|(_, name)| name.as_str()).collect::<Vec<_>>(),
    ["Preview release"]
  );
  assert_eq!(
    resources(&repo, ResourceKind::Instruction).len(),
    1,
    "Release rules came along"
  );
  let db = ProjectDb::open(&repo).expect("open Kontur");
  let (pipeline, builder) = (pipelines[0].0, profiles[0].0);
  assert_eq!(
    db.latest_version::<Pipeline>(pipeline).expect("read"),
    None,
    "imported as a draft"
  );
  let draft = db.draft::<Pipeline>(pipeline).expect("read").expect("a draft");
  assert_eq!(draft.body().steps()[0].kind.executor(), Some(builder));
  let profile = db.draft::<AgentProfile>(builder).expect("read").expect("a draft");
  assert_eq!(profile.body().connection(), own, "Kontur's own connection");
  assert!(
    desktop.tree.get_element_by_id("import-dialog").is_none(),
    "the dialog closed"
  );
  let imported = desktop.activity.snapshot();
  let event = imported.recent().into_iter().next().expect("the import is listed");
  assert_eq!(event.title().key, "statusBar.operations.import.done.other");
  assert_eq!(
    event.target(),
    &Target::Project {
      repo_root: repo,
      place: Place::Pipeline(pipeline)
    }
  );
  assert_eq!(
    fingerprint(&desktop.other_repo()),
    before,
    "Personal website was only read"
  );
}

#[test]
fn import_needs_a_rename_when_kontur_has_the_name() {
  let (mut desktop, source, _) = resolving(|repo| {
    let mut db = ProjectDb::open(repo).expect("open Kontur");
    let own = Pipeline::new(Vec::new()).expect("pipeline");
    db.create_resource(
      Slug::new("preview-release").expect("slug"),
      name("Preview release"),
      &own,
      Timestamp::now(),
    )
    .expect("Kontur's own pipeline");
  });
  let pipeline = format!("import-status-{}", source.pipeline);
  assert!(shows(&mut desktop, &pipeline, "Name taken"));
  assert!(shows(
    &mut desktop,
    "import-readiness",
    "Rename Preview release to continue."
  ));

  let choice = format!("import-choice-{}", source.pipeline);
  choose(&mut desktop, &choice, "Rename copy");
  let rename = format!("import-rename-{}", source.pipeline);
  desktop.wait_for(&rename);
  // The name is in the field beside the select, which reads just Rename copy (06.C `FKzQ9`). The
  // field starts with the source's name, which Kontur has: the field shows the error.
  assert_eq!(selected(&mut desktop, &choice), ["Rename copy"]);
  assert!(shows(&mut desktop, &pipeline, "Name taken"));
  assert!(invalid(&mut desktop, &rename));
  type_name(&mut desktop, &rename, "");
  assert!(shows(&mut desktop, &pipeline, "Name needed"));
  assert!(shows(&mut desktop, &pipeline, "Enter a name for the copy"));
  assert!(shows(
    &mut desktop,
    "import-readiness",
    "Name the copy of Preview release to continue."
  ));
  assert!(invalid(&mut desktop, &rename));
  type_name(&mut desktop, &rename, "Website preview");
  assert!(shows(&mut desktop, &pipeline, "Name resolved"));
  assert!(!invalid(&mut desktop, &rename));
  assert_eq!(selected(&mut desktop, &choice), ["Rename copy"]);
  desktop.press("import-submit");
  desktop.wait_for("pipeline-state");

  let names: BTreeSet<String> = resources(desktop.repo(), ResourceKind::Pipeline)
    .into_iter()
    .map(|(_, name)| name)
    .collect();
  assert_eq!(
    names,
    BTreeSet::from(["Preview release".to_owned(), "Website preview".to_owned()])
  );
}

#[test]
fn import_closes_back_to_the_page_it_was_opened_over() {
  let (mut desktop, ..) = resolving(|_| {});

  desktop.press("import-close");

  assert!(desktop.tree.get_element_by_id("import-dialog").is_none());
  assert!(desktop.tree.get_element_by_id("nav-import").is_some());
  assert_eq!(
    resources(desktop.repo(), ResourceKind::Pipeline),
    [],
    "closing imports nothing"
  );
}

/// Kontur with Personal website beside it, the import open from the sidebar, listing Personal
/// website's resources.
fn opened() -> (Desktop, Source) {
  let mut source = None;
  let mut desktop = Desktop::with_other_project(|repo| source = Some(seed_source(repo)), |_| {});
  let source = source.expect("seeded");
  desktop.wait_for("nav-import");
  desktop.press("nav-import");
  desktop.wait_for(&format!("import-resource-{}", source.pipeline));
  (desktop, source)
}

#[test]
fn import_escape_closes_it_like_every_dialog() {
  let (mut desktop, _) = opened();

  desktop.key("Escape", "Escape");

  assert!(desktop.tree.get_element_by_id("import-dialog").is_none());
  assert!(desktop.tree.get_element_by_id("nav-import").is_some());
  // Closed for good: the page under it does not open it again.
  desktop.pass();
  assert!(desktop.tree.get_element_by_id("import-dialog").is_none());
}

#[test]
fn import_tab_reaches_the_kind_tabs_between_the_source_and_the_search() {
  let (mut desktop, source) = opened();
  desktop.tab_to("import-tab-all", 10);
  let order = [
    Some("import-tab-pipelines"),
    Some("import-tab-agents"),
    Some("import-tab-instructions"),
    Some("import-tab-tools"),
    // Secrets cannot be imported: its tab is no stop.
    Some("import-search"),
  ];
  assert_eq!(desktop.stops(order.len(), false), expected(&order));

  // Enter selects the focused tab, which keeps the focus.
  desktop.tab_to("import-tab-agents", 20);
  desktop.enter();
  assert_eq!(desktop.focused().as_deref(), Some("import-tab-agents"));
  assert!(
    desktop
      .tree
      .get_element_by_id(&format!("import-resource-{}", source.pipeline))
      .is_none(),
    "the Agents tab lists no pipeline"
  );
}
