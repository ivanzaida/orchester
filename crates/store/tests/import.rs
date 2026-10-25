//! Import resources: another project's database is only read, and the copies are written into
//! this project in one transaction, once per import.

#[path = "import/record.rs"]
mod record;

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::{Path, PathBuf},
  time::SystemTime,
};

use orchester_core::{
  connection::{
    AgentEndpoint, AgentProvider, Connection, ConnectionKind,
    mcp::{
      HttpServer, McpAuthorization, McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, McpUrl,
    },
  },
  id::{ConnectionId, IdempotencyKey, ProjectId, ResourceId},
  import::{Catalog, Choice, Choices, Entity, ImportBatch, Pick, Plan},
  name::Name,
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, AgentTargets, InjectionEvent, Instruction, InstructionScope, ModelId, Pipeline, PromptText,
    ProviderSettings, RepositoryAccess, ResourceKind, Slug, Step, StepKey, StepKind, VersionNumber,
  },
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::{ConnectionStore, ImportStore, McpServerStore, ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn repository(title: &str, key: &str) -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(ProjectId::generate(), ProjectKey::new(key).expect("key"), name(title));
  let db = ProjectDb::create(repo.path(), project).expect("create the project database");
  (repo, db)
}

fn codex(db: &mut ProjectDb) -> ConnectionId {
  let connection = Connection::new(
    ConnectionId::generate(),
    name("Codex"),
    ConnectionKind::Agent(AgentProvider::CodexCli),
    AgentEndpoint::Program(std::env::temp_dir().join("codex")),
    at(1),
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
      McpAuthorization::None,
    )),
    McpEnvironment::default(),
    at(1),
  )
  .expect("valid server");
  db.create_mcp_server(&server).expect("stored");
  server.id()
}

#[test]
fn import_profile_uses_the_target_mcp_server() {
  let (source_repo, mut source_db) = repository("Personal website", "PW");
  let source_connection = codex(&mut source_db);
  let source_server = mcp(&mut source_db, "source-docs");
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    source_connection,
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  )
  .with_mcp_servers(McpServerSelection::new(vec![source_server]).expect("one server"));
  let profile_id = source_db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, at(2))
    .expect("profile")
    .id();
  drop(source_db);
  let source = catalog_of(source_repo.path());
  let (_target_repo, mut target_db) = repository("Kontur", "KON");
  let target_server = mcp(&mut target_db, "target-docs");
  let target = Catalog::read(target_db.project().clone(), &target_db).expect("target catalog");
  let mut choices = Choices::default();
  choices.choose(
    Entity::McpServer(source_server),
    Choice::Use(Entity::McpServer(target_server)),
  );
  let plan = Plan::new(&source, &target, &[(profile_id, Pick::Draft)], &choices);
  let batch = plan.batch(&source, IdempotencyKey::generate(), at(3)).expect("ready");

  let created = target_db.import(&batch).expect("imported");
  let draft = target_db
    .draft::<AgentProfile>(created[0].id())
    .expect("read")
    .expect("imported draft");
  assert_eq!(draft.body().mcp_servers().ids(), [target_server]);
  assert_ne!(draft.body().mcp_servers().ids(), [source_server]);
  assert_eq!(target_db.mcp_servers().expect("local servers").len(), 1);

  let gone = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("gone-docs").expect("name"),
    McpTransport::Http(HttpServer::new(
      McpUrl::new("https://mcp.example.test/v1").expect("URL"),
      McpAuthorization::None,
    )),
    McpEnvironment::default(),
    at(1),
  )
  .expect("server");
  let stale_target = Catalog::new(
    target_db.project().clone(),
    Vec::new(),
    Vec::new(),
    vec![gone.clone()],
    Vec::new(),
  );
  choices.choose(
    Entity::McpServer(source_server),
    Choice::Use(Entity::McpServer(gone.id())),
  );
  let stale_plan = Plan::new(&source, &stale_target, &[(profile_id, Pick::Draft)], &choices);
  let stale_batch = stale_plan
    .batch(&source, IdempotencyKey::generate(), at(4))
    .expect("ready");
  let refused = target_db.import(&stale_batch);
  assert!(matches!(refused, Err(StoreError::ConnectionNotFound(id)) if id == gone.id()));
  assert_eq!(
    count(&target_db, ResourceKind::AgentProfile),
    1,
    "stale mapping writes nothing"
  );
}

/// Personal website: the Preview release pipeline (published v1) runs Builder (a draft), whose
/// connection is Codex, and has Release rules; a secret nothing uses.
fn personal_website() -> (TempDir, ResourceId) {
  let (repo, mut db) = repository("Personal website", "PW");
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
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, at(2))
    .expect("profile")
    .id();
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
  let pipeline = Pipeline::new(vec![step]).expect("pipeline");
  let preview = db
    .create_resource(
      Slug::new("preview-release").expect("slug"),
      name("Preview release"),
      &pipeline,
      at(3),
    )
    .expect("pipeline")
    .id();
  db.publish_draft::<Pipeline>(preview, at(4)).expect("published");
  let rules = Instruction::new(
    PromptText::new("Keep the changelog.").expect("prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Pipeline { pipeline: preview },
  )
  .expect("instruction");
  db.create_resource(
    Slug::new("release-rules").expect("slug"),
    name("Release rules"),
    &rules,
    at(5),
  )
  .expect("instruction");
  let secret = Secret::new(
    orchester_core::id::SecretId::generate(),
    NewSecret {
      name: SecretName::new("PREVIEW_HOST_API_KEY").expect("secret name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Staging,
    },
    at(6),
  );
  db.create_secret(&secret).expect("secret");
  // Closing the database checkpoints its log into the file, as a project not open elsewhere is.
  drop(db);
  (repo, preview)
}

fn catalog_of(repo: &Path) -> Catalog {
  let db = ProjectDb::open_read_only(repo).expect("the source opens to be read");
  Catalog::read(db.project().clone(), &*db).expect("the source reads")
}

fn database(repo: &Path) -> PathBuf {
  repo.join(".orchester").join("orchester.db")
}

fn fingerprint(repo: &Path) -> (Vec<u8>, SystemTime) {
  let path = database(repo);
  let hash = Sha256::digest(fs::read(&path).expect("read the database file")).to_vec();
  let modified = fs::metadata(&path)
    .and_then(|meta| meta.modified())
    .expect("modified time");
  (hash, modified)
}

/// Preview release v1 into `target`, Builder's connection mapped to `connection`.
fn batch(source: &Catalog, target: &ProjectDb, preview: ResourceId, connection: Option<ConnectionId>) -> ImportBatch {
  let target = Catalog::read(target.project().clone(), target).expect("the target reads");
  let mut choices = Choices::default();
  if let Some(connection) = connection {
    let codex = source.connections()[0].id();
    choices.choose(Entity::Connection(codex), Choice::Use(Entity::Connection(connection)));
  }
  let first = VersionNumber::FIRST;
  let plan = Plan::new(source, &target, &[(preview, Pick::Published(first))], &choices);
  plan.batch(source, IdempotencyKey::generate(), at(10)).expect("ready")
}

fn count(db: &ProjectDb, kind: ResourceKind) -> usize {
  db.resources(kind).expect("listed").len()
}

#[test]
fn import_writes_every_copy_as_a_draft_with_its_origin() {
  let (source_repo, preview) = personal_website();
  let source = catalog_of(source_repo.path());
  let (target_repo, mut target) = repository("Kontur", "KON");
  let connection = codex(&mut target);

  let created = target
    .import(&batch(&source, &target, preview, Some(connection)))
    .expect("imported");

  let kinds: Vec<_> = created.iter().map(|resource| resource.kind()).collect();
  assert_eq!(
    kinds,
    [
      ResourceKind::Pipeline,
      ResourceKind::AgentProfile,
      ResourceKind::Instruction
    ]
  );
  for resource in &created {
    assert_eq!(target.resource(resource.id()).expect("read").as_ref(), Some(resource));
  }
  let pipeline = created[0].id();
  assert_eq!(
    target.latest_version::<Pipeline>(pipeline).expect("read"),
    None,
    "nothing is published"
  );
  let draft = target.draft::<Pipeline>(pipeline).expect("read").expect("a draft");
  assert_eq!(draft.based_on(), None);
  assert_eq!(draft.body().steps()[0].kind.executor(), Some(created[1].id()));
  let profile = target
    .draft::<AgentProfile>(created[1].id())
    .expect("read")
    .expect("a draft");
  assert_eq!(profile.body().connection(), connection);
  let origin: (String, String, String, Option<i64>, i64) = rusqlite::Connection::open(database(target_repo.path()))
    .expect("open")
    .query_row(
      "SELECT source_project_id, source_project_name, source_resource_id, source_version, imported_at \
       FROM resource_imports WHERE resource_id = ?1",
      [pipeline.to_string()],
      |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    )
    .expect("the pipeline's origin");
  assert_eq!(
    origin,
    (
      source.project().id().to_string(),
      "Personal website".to_owned(),
      preview.to_string(),
      Some(1),
      10
    )
  );
}

#[test]
fn import_leaves_the_source_database_untouched() {
  let (source_repo, preview) = personal_website();
  let before = fingerprint(source_repo.path());
  let (_target_repo, mut target) = repository("Kontur", "KON");

  let source = catalog_of(source_repo.path());
  target
    .import(&batch(&source, &target, preview, None))
    .expect("imported");

  assert_eq!(
    fingerprint(source_repo.path()),
    before,
    "same content and modification time"
  );
  let reopened = ProjectDb::open(source_repo.path()).expect("the source still opens");
  assert_eq!(
    count(&reopened, ResourceKind::Pipeline),
    1,
    "nothing was added to the source"
  );
}

#[test]
fn import_with_a_key_already_written_creates_nothing_new() {
  let (source_repo, preview) = personal_website();
  let source = catalog_of(source_repo.path());
  let (_target_repo, mut target) = repository("Kontur", "KON");
  let batch = batch(&source, &target, preview, None);

  let first = target.import(&batch).expect("imported");
  let again = target.import(&batch).expect("the retry succeeds");

  assert_eq!(again, first);
  assert_eq!(count(&target, ResourceKind::Pipeline), 1);
  assert_eq!(count(&target, ResourceKind::AgentProfile), 1);
}

#[test]
fn import_writes_nothing_when_a_mapped_connection_is_gone() {
  let (source_repo, preview) = personal_website();
  let source = catalog_of(source_repo.path());
  let (_target_repo, mut target) = repository("Kontur", "KON");
  let gone = ConnectionId::generate();
  let target_catalog = Catalog::read(target.project().clone(), &target).expect("read");
  let with_gone = Catalog::new(
    target_catalog.project().clone(),
    Vec::new(),
    vec![
      Connection::new(
        gone,
        name("Codex"),
        ConnectionKind::Agent(AgentProvider::CodexCli),
        AgentEndpoint::Program(std::env::temp_dir().join("codex")),
        at(1),
      )
      .expect("connection"),
    ],
    Vec::new(),
    Vec::new(),
  );
  let mut choices = Choices::default();
  choices.choose(
    Entity::Connection(source.connections()[0].id()),
    Choice::Use(Entity::Connection(gone)),
  );
  let plan = Plan::new(
    &source,
    &with_gone,
    &[(preview, Pick::Published(VersionNumber::FIRST))],
    &choices,
  );
  let batch = plan.batch(&source, IdempotencyKey::generate(), at(10)).expect("ready");

  let refused = target.import(&batch);

  assert!(
    matches!(refused, Err(StoreError::ConnectionNotFound(id)) if id == gone),
    "{refused:?}"
  );
  assert_eq!(count(&target, ResourceKind::Pipeline), 0);
  assert_eq!(count(&target, ResourceKind::AgentProfile), 0);
}

#[test]
fn import_refuses_a_slug_taken_since_the_plan() {
  let (source_repo, preview) = personal_website();
  let source = catalog_of(source_repo.path());
  let (_target_repo, mut target) = repository("Kontur", "KON");
  let batch = batch(&source, &target, preview, None);
  let taken = Instruction::new(
    PromptText::new("Ours.").expect("prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("instruction");
  target
    .create_resource(
      Slug::new("release-rules").expect("slug"),
      name("Our rules"),
      &taken,
      at(9),
    )
    .expect("created meanwhile");

  let refused = target.import(&batch);

  assert!(
    matches!(&refused, Err(StoreError::DuplicateSlug { slug, .. }) if slug == "release-rules"),
    "{refused:?}"
  );
  assert_eq!(
    count(&target, ResourceKind::Pipeline),
    0,
    "the transaction wrote nothing"
  );
}

#[test]
fn import_source_not_migrated_yet_is_refused_and_left_as_it_is() {
  let repo = tempfile::tempdir().expect("temporary directory");
  let old = ProjectDb::create_at_version(repo.path(), 1).expect("an old database");
  old
    .execute(
      "INSERT INTO project (id, key, name) VALUES (?1, 'PW', 'Personal website')",
      [ProjectId::generate().to_string()],
    )
    .expect("project row");
  drop(old);
  let before = fingerprint(repo.path());

  let refused = ProjectDb::open_read_only(repo.path());

  assert!(
    matches!(refused, Err(StoreError::SchemaTooOld { found: 1, .. })),
    "{:?}",
    refused.err()
  );
  assert_eq!(fingerprint(repo.path()), before);
}

#[test]
fn import_source_without_a_project_is_refused() {
  let repo = tempfile::tempdir().expect("temporary directory");

  let refused = ProjectDb::open_read_only(repo.path());

  assert!(
    matches!(refused, Err(StoreError::NotInitialized(_))),
    "{:?}",
    refused.err()
  );
  assert!(!database(repo.path()).exists(), "nothing was created");
}

#[test]
fn import_reads_the_source_catalog() {
  let (source_repo, preview) = personal_website();

  let source = catalog_of(source_repo.path());

  let names: BTreeMap<_, _> = source
    .resources()
    .iter()
    .map(|entry| (entry.resource().name().as_str().to_owned(), entry.default_pick()))
    .collect();
  assert_eq!(
    names,
    BTreeMap::from([
      ("Builder".to_owned(), Pick::Draft),
      ("Preview release".to_owned(), Pick::Published(VersionNumber::FIRST)),
      ("Release rules".to_owned(), Pick::Draft),
    ])
  );
  assert_eq!(source.resources()[0].resource().id(), preview, "pipelines come first");
  assert_eq!(source.connections().len(), 1);
  assert_eq!(source.secrets().len(), 1);
}

#[test]
fn import_reads_a_source_open_in_another_process() {
  let (source_repo, _preview) = personal_website();
  let mut writer = ProjectDb::open(source_repo.path()).expect("open, as the source's runner has it");
  let profile = AgentProfile::new(
    name("Reviewer"),
    String::new(),
    PromptText::new("Review.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  );
  writer
    .create_resource(Slug::new("reviewer").expect("slug"), name("Reviewer"), &profile, at(7))
    .expect("created while the source is open");

  let source = catalog_of(source_repo.path());

  let names: Vec<_> = source
    .resources()
    .iter()
    .map(|entry| entry.resource().name().as_str())
    .collect();
  assert!(
    names.contains(&"Reviewer"),
    "committed changes still in the log are read: {names:?}"
  );
}
