//! Which project resources appear in a secret's read-only Used by list.

#[path = "secrets/replacement.rs"]
mod replacement;

use std::{collections::BTreeMap, fs};

use orchester_core::{
  id::{ProjectId, SecretId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{
    AccessPolicy, AccessWindow, ApprovalMode, DeniedBehavior, HttpAuth, HttpMethod, HttpRequest, Slug, Tool, ToolAction,
  },
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::{ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  connections::mcp::{McpServerText, TransportChoice, save_in, secret_reference, set_enabled_in},
  secrets::data::{ProjectSecrets, SecretUseKind, UseCounts},
};
use tempfile::TempDir;

fn project_db() -> (TempDir, ProjectDb) {
  let scratch = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/secrets-tests");
  fs::create_dir_all(&scratch).expect("test scratch");
  let repo = TempDir::new_in(scratch).expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Project").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create project database");
  (repo, db)
}

fn secret(name: &str) -> Secret {
  Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new(name).expect("valid name"),
      kind: SecretKind::AccessToken,
      environment: Environment::Staging,
    },
    Timestamp::from_unix_millis(1),
  )
}

#[test]
fn mcp_variable_and_bearer_references_appear_once_per_server() {
  let (repo, mut db) = project_db();
  let variable = secret("DOCS_TOKEN");
  let bearer = secret("REMOTE_TOKEN");
  let unused = secret("UNUSED_TOKEN");
  for secret in [&variable, &bearer, &unused] {
    db.create_secret(secret).expect("secret metadata");
  }

  let reference = secret_reference(&variable);
  let stdio = McpServerText {
    name: "docs".to_owned(),
    program: std::env::current_exe()
      .expect("test executable")
      .to_string_lossy()
      .into_owned(),
    variables: format!("FIRST={reference}\nSECOND={reference}"),
    ..McpServerText::new_server()
  };
  let stdio_id = save_in(
    repo.path(),
    None,
    stdio
      .draft(&[], std::slice::from_ref(&variable))
      .expect("valid stdio server"),
  )
  .expect("save stdio server");
  let remote = McpServerText {
    name: "remote".to_owned(),
    transport: TransportChoice::Http,
    url: "https://mcp.example.test".to_owned(),
    bearer: Some(bearer.id()),
    ..McpServerText::new_server()
  };
  let remote_id = save_in(
    repo.path(),
    None,
    remote
      .draft(&[], std::slice::from_ref(&bearer))
      .expect("valid remote server"),
  )
  .expect("save remote server");
  set_enabled_in(repo.path(), stdio_id, false).expect("disable stdio server");

  let data = ProjectSecrets::load(&db).expect("read secrets and uses");
  assert_eq!(
    data.use_counts(variable.id()),
    UseCounts {
      tools: 0,
      mcp_servers: 1
    }
  );
  assert_eq!(
    data.use_counts(bearer.id()),
    UseCounts {
      tools: 0,
      mcp_servers: 1
    }
  );
  assert_eq!(data.use_counts(unused.id()), UseCounts::default());
  let variable_uses: Vec<_> = data.used_by(variable.id()).collect();
  assert_eq!(variable_uses.len(), 1, "two variables on one server count once");
  assert_eq!(variable_uses[0].kind, SecretUseKind::McpServer(stdio_id));
  assert_eq!(&*variable_uses[0].name, "docs");
  let bearer_uses: Vec<_> = data.used_by(bearer.id()).collect();
  assert_eq!(bearer_uses.len(), 1);
  assert_eq!(bearer_uses[0].kind, SecretUseKind::McpServer(remote_id));
}

#[test]
fn tool_and_mcp_server_using_one_secret_have_separate_counts() {
  let (repo, mut db) = project_db();
  let shared = secret("SHARED_TOKEN");
  db.create_secret(&shared).expect("secret metadata");

  let request = HttpRequest::new(
    HttpMethod::Post,
    "https://deploy.example.test/v1".into(),
    "deploy.example.test".into(),
    HttpAuth::Bearer { secret: shared.id() },
    BTreeMap::new(),
    BTreeMap::new(),
  )
  .expect("valid request");
  let policy = AccessPolicy::new(
    BTreeMap::new(),
    Name::new("Staging").expect("valid environment"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .expect("valid policy");
  let tool = Tool::new("", ToolAction::Http(request), Some(policy)).expect("valid tool");
  let tool_id = db
    .create_resource(
      Slug::new("deploy").expect("valid slug"),
      Name::new("Deploy").expect("valid name"),
      &tool,
      Timestamp::from_unix_millis(2),
    )
    .expect("save tool")
    .id();

  let reference = secret_reference(&shared);
  let server = McpServerText {
    name: "docs".to_owned(),
    program: std::env::current_exe()
      .expect("test executable")
      .to_string_lossy()
      .into_owned(),
    variables: format!("FIRST={reference}\nSECOND={reference}"),
    ..McpServerText::new_server()
  };
  let server_id = save_in(
    repo.path(),
    None,
    server.draft(&[], std::slice::from_ref(&shared)).expect("valid server"),
  )
  .expect("save server");

  let data = ProjectSecrets::load(&db).expect("read secrets and uses");
  let counts = data.use_counts(shared.id());
  assert_eq!(
    counts,
    UseCounts {
      tools: 1,
      mcp_servers: 1
    }
  );
  assert_eq!(counts.total(), 2);
  let uses: Vec<_> = data.used_by(shared.id()).collect();
  assert_eq!(uses.len(), 2, "one tool and one server despite repeated MCP variables");
  assert_eq!(uses[0].kind, SecretUseKind::Tool(tool_id));
  assert_eq!(&*uses[0].name, "Deploy");
  assert_eq!(uses[1].kind, SecretUseKind::McpServer(server_id));
  assert_eq!(&*uses[1].name, "docs");
}

#[test]
fn secrets_name_check_is_typed_environment_scoped_and_ignores_archives() {
  use orchester_core::secret::SecretNameError;
  use orchester_workspace::secrets::form::{NameCheck, check_name};
  let existing = secret("DOCS_TOKEN");
  let secrets = [existing.clone()];
  assert_eq!(check_name(&secrets, "  ", Environment::Staging), NameCheck::Empty);
  assert!(matches!(
    check_name(&secrets, "lower_case", Environment::Staging),
    NameCheck::Invalid(_)
  ));
  assert!(matches!(
    check_name(&secrets, &"A".repeat(129), Environment::Staging),
    NameCheck::Invalid(SecretNameError::TooLong { .. })
  ));
  assert_eq!(
    check_name(&secrets, " DOCS_TOKEN ", Environment::Staging),
    NameCheck::Taken(existing.clone())
  );
  assert!(
    check_name(&secrets, "DOCS_TOKEN", Environment::Production)
      .new_secret(SecretKind::ApiKey, Environment::Production)
      .is_some()
  );
  let archived = Secret::from_stored(
    existing.id(),
    NewSecret {
      name: existing.name().clone(),
      kind: existing.kind(),
      environment: existing.environment(),
    },
    existing.created_at(),
    None,
    Some(Timestamp::from_unix_millis(3)),
  );
  assert!(matches!(
    check_name(&[archived], "DOCS_TOKEN", Environment::Staging),
    NameCheck::Free(_)
  ));
}

#[test]
fn secrets_selected_archive_keeps_uses_without_returning_to_the_active_list() {
  let (repo, mut db) = project_db();
  let active = secret("DOCS_TOKEN");
  db.create_secret(&active).expect("metadata");
  let server = McpServerText {
    name: "docs".to_owned(),
    program: std::env::current_exe()
      .expect("test executable")
      .to_string_lossy()
      .into_owned(),
    variables: format!("TOKEN={}", secret_reference(&active)),
    ..McpServerText::new_server()
  };
  save_in(
    repo.path(),
    None,
    server.draft(&[], std::slice::from_ref(&active)).expect("draft"),
  )
  .expect("server");
  let archived = Secret::from_stored(
    active.id(),
    NewSecret {
      name: active.name().clone(),
      kind: active.kind(),
      environment: active.environment(),
    },
    active.created_at(),
    None,
    Some(Timestamp::from_unix_millis(3)),
  );
  db.save_secret(&archived).expect("archive metadata");
  assert!(ProjectSecrets::load(&db).expect("active").secrets.is_empty());
  let data = ProjectSecrets::load_selected(&db, Some(active.id())).expect("selected archive");
  assert!(data.secret(active.id()).expect("archived metadata").is_archived());
  assert_eq!(data.in_environment(None).count(), 0);
  assert_eq!(data.used_by(active.id()).count(), 1);
  assert!(data.replacement_needs_person(active.id()));
}

#[derive(Default)]
struct CountingVault(std::sync::atomic::AtomicUsize);

impl orchester_core::secret::SecretVault for CountingVault {
  fn store(
    &self,
    _key: &orchester_core::secret::VaultKey,
    _value: &orchester_core::secret::SecretValue,
  ) -> Result<(), orchester_core::secret::VaultError> {
    self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    Ok(())
  }

  fn read(
    &self,
    _key: &orchester_core::secret::VaultKey,
  ) -> Result<orchester_core::secret::SecretValue, orchester_core::secret::VaultError> {
    Err(orchester_core::secret::VaultError::NotFound)
  }

  fn delete(&self, _key: &orchester_core::secret::VaultKey) -> Result<(), orchester_core::secret::VaultError> {
    panic!("replacement must not delete a credential")
  }
}

#[test]
fn secrets_replacement_checks_current_bindings_before_touching_the_vault() {
  use std::sync::atomic::Ordering;

  use orchester_core::secret::SecretValue;
  use orchester_workspace::secrets::data::{ReplacementOutcome, ReplacementPermission, replace_allowed_in};

  let (repo, mut db) = project_db();
  let active = secret("DOCS_TOKEN");
  db.create_secret(&active).expect("metadata");
  let snapshot = ProjectSecrets::load(&db).expect("unused snapshot");
  assert!(!snapshot.replacement_needs_person(active.id()));
  let vault = CountingVault::default();
  let value = SecretValue::new("fixture-only-replacement".into()).expect("fixture");
  assert_eq!(
    replace_allowed_in(
      repo.path(),
      &vault,
      active.id(),
      &value,
      ReplacementPermission::UnusedOnly
    )
    .expect("unused"),
    ReplacementOutcome::Replaced
  );
  let rotated = db.secret(active.id()).expect("metadata").expect("secret").rotated_at();
  let server = McpServerText {
    name: "remote".into(),
    transport: TransportChoice::Http,
    url: "https://mcp.example.test".into(),
    bearer: Some(active.id()),
    ..McpServerText::new_server()
  };
  save_in(
    repo.path(),
    None,
    server.draft(&[], std::slice::from_ref(&active)).expect("draft"),
  )
  .expect("server");
  assert!(!snapshot.replacement_needs_person(active.id()), "snapshot is stale");
  assert_eq!(
    replace_allowed_in(
      repo.path(),
      &vault,
      active.id(),
      &value,
      ReplacementPermission::UnusedOnly
    )
    .expect("recheck"),
    ReplacementOutcome::NeedsPerson
  );
  assert_eq!(
    vault.0.load(Ordering::SeqCst),
    1,
    "no additional vault write without confirmation"
  );
  assert_eq!(
    db.secret(active.id()).expect("metadata").expect("secret").rotated_at(),
    rotated
  );
  assert_eq!(
    replace_allowed_in(
      repo.path(),
      &vault,
      active.id(),
      &value,
      ReplacementPermission::PersonConfirmed
    )
    .expect("confirmed"),
    ReplacementOutcome::Replaced
  );
  assert_eq!(vault.0.load(Ordering::SeqCst), 2);
}
