//! Agent profile editor logic: the form, the model choices of a connection and the
//! Draft saved → Publish vN → Published vN lifecycle through the project database.

use std::{fs, num::NonZeroU32};

use orchester_core::{
  connection::{
    AgentProvider, CatalogModel, CatalogSource, CheckResult, Connection, ConnectionCheck, ModelCatalog, ModelSupport,
    REASONING_EFFORT, SignIn,
  },
  id::{ConnectionId, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{AgentProfile, Draft, ModelId, PublishError, ResourceKind, VersionNumber},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_desktop::agents::{
  data::{ProjectAgents, VersionState, create_profile, keep_draft_over_latest, publish, save_draft},
  form::{ModelSelection, ProfileForm, effort_options, model_options, stored_effort},
};
use orchester_store::{ProjectDb, StoreError};
use tempfile::TempDir;

fn project_db() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create the project database");
  (repo, db)
}

fn model(id: &str) -> ModelId {
  ModelId::new(id).expect("valid model id")
}

fn now() -> Timestamp {
  Timestamp::from_unix_millis(1_700_000_000_000)
}

fn version(number: u32) -> VersionNumber {
  VersionNumber::new(NonZeroU32::new(number).expect("non-zero"))
}

/// A checked Codex connection whose catalog lists two models.
fn codex(repo: &TempDir) -> Connection {
  let mut connection = Connection::new(
    ConnectionId::generate(),
    Name::new("Codex").expect("valid name"),
    AgentProvider::CodexCli,
    repo.path().join("codex.exe"),
    now(),
  )
  .expect("absolute executable");
  let catalog = ModelCatalog::new(
    CatalogSource::Provider,
    vec![
      CatalogModel::new(model("gpt-5.5"), vec!["low".into(), "high".into()]),
      CatalogModel::new(model("gpt-5.4-mini"), Vec::new()),
    ],
  );
  connection.record_check(ConnectionCheck {
    at: now(),
    result: CheckResult::Available {
      version: "0.139.0".into(),
      sign_in: SignIn::SignedIn,
      catalog,
    },
  });
  connection
}

fn form<'a>(connection: &'a Connection, selection: &'a ModelSelection, effort: Option<&'a str>) -> ProfileForm<'a> {
  ProfileForm {
    role: " Builder ",
    responsibilities: "Implement the approved plan",
    instructions: "Build {{task.title}}.",
    connection: Some(connection),
    model: selection,
    custom_model: "",
    effort,
  }
}

#[test]
fn model_options_come_from_the_connection_catalog() {
  let (repo, _db) = project_db();
  let codex = codex(&repo);
  let listed: Vec<(String, ModelSupport)> = model_options(&codex, Some(&model("gpt-5.5")))
    .into_iter()
    .map(|option| (option.id.as_str().to_owned(), option.support))
    .collect();
  assert_eq!(
    listed,
    [
      ("gpt-5.5".to_owned(), ModelSupport::Listed),
      ("gpt-5.4-mini".to_owned(), ModelSupport::Listed)
    ]
  );
  let with_custom = model_options(&codex, Some(&model("my-fine-tune")));
  let last = with_custom.last().expect("options");
  assert_eq!(last.id.as_str(), "my-fine-tune");
  assert_eq!(last.support, ModelSupport::NotVerified);
  assert_eq!(effort_options(&codex, &model("gpt-5.5")), ["low", "high"]);
  assert!(effort_options(&codex, &model("my-fine-tune")).is_empty());
}

#[test]
fn profile_form_builds_a_profile_and_keeps_only_offered_efforts() {
  let (repo, _db) = project_db();
  let codex = codex(&repo);
  let listed = ModelSelection::Listed(model("gpt-5.5"));
  let profile = form(&codex, &listed, Some("high")).profile().expect("valid form");
  assert_eq!(profile.role().as_str(), "Builder");
  assert_eq!(profile.connection(), codex.id());
  assert_eq!(stored_effort(&profile).as_deref(), Some("high"));

  let no_efforts = ModelSelection::Listed(model("gpt-5.4-mini"));
  let profile = form(&codex, &no_efforts, Some("high")).profile().expect("valid form");
  assert_eq!(profile.settings().iter().count(), 0);
  assert!(REASONING_EFFORT.contains("effort"));
}

#[test]
fn profile_form_reports_each_problem() {
  let (repo, _db) = project_db();
  let codex = codex(&repo);
  let custom = ModelSelection::Custom;
  let errors = ProfileForm {
    role: "  ",
    responsibilities: "",
    instructions: "Use {{task.nope}} and {{task.title",
    connection: None,
    model: &custom,
    custom_model: "bad model",
    effort: None,
  }
  .profile()
  .expect_err("invalid form");
  assert_eq!(errors.role.map(|message| message.key), Some("errors.roleRequired"));
  let instructions = errors.instructions.expect("instructions error");
  assert_eq!(instructions.key, "errors.unknownVariable");
  assert_eq!(instructions.args, [("name", "task.nope".to_owned())]);
  assert_eq!(
    errors.connection.map(|message| message.key),
    Some("errors.connectionRequired")
  );
  assert_eq!(errors.model.map(|message| message.key), Some("errors.modelInvalid"));

  let typed = ProfileForm {
    custom_model: "my-fine-tune",
    ..form(&codex, &custom, None)
  };
  assert_eq!(typed.profile().expect("valid").model().as_str(), "my-fine-tune");
}

#[test]
fn profile_lifecycle_draft_publish_and_next_draft() {
  let (repo, mut db) = project_db();
  let codex = codex(&repo);
  db.create_connection(&codex).expect("store the connection");
  let listed = ModelSelection::Listed(model("gpt-5.5"));
  let profile = form(&codex, &listed, None).profile().expect("valid form");

  let id = create_profile(&mut db, &profile, now()).expect("create");
  let agents = ProjectAgents::load(&db).expect("load");
  assert_eq!(agents.connections.len(), 1);
  let entry = agents.profile(id).expect("created profile");
  assert_eq!(entry.state, VersionState::Draft { published: None });
  assert_eq!(
    db.resource(id).expect("read").expect("exists").slug().as_str(),
    "builder"
  );

  assert_eq!(publish(&mut db, id, now()).expect("publish"), version(1));
  let entry = ProjectAgents::load(&db)
    .expect("load")
    .profile(id)
    .cloned()
    .expect("profile");
  assert_eq!(entry.state, VersionState::Published(version(1)));
  assert!(matches!(publish(&mut db, id, now()), Err(StoreError::NoDraft(_))));

  let edited = ProfileForm {
    responsibilities: "Implement and test the approved plan",
    ..form(&codex, &listed, Some("low"))
  }
  .profile()
  .expect("valid form");
  save_draft(&mut db, id, edited.clone(), now()).expect("save the next draft");
  let entry = ProjectAgents::load(&db)
    .expect("load")
    .profile(id)
    .cloned()
    .expect("profile");
  assert_eq!(
    entry.state,
    VersionState::Draft {
      published: Some(version(1))
    }
  );
  assert_eq!(entry.profile, edited);
  assert_eq!(publish(&mut db, id, now()).expect("publish"), version(2));
}

#[test]
fn stale_draft_is_refused_until_kept_over_the_latest_version() {
  let (repo, mut db) = project_db();
  let codex = codex(&repo);
  let listed = ModelSelection::Listed(model("gpt-5.5"));
  let profile = form(&codex, &listed, None).profile().expect("valid form");
  let id = create_profile(&mut db, &profile, now()).expect("create");
  publish(&mut db, id, now()).expect("publish v1");
  let mini = ModelSelection::Listed(model("gpt-5.4-mini"));
  let edited = form(&codex, &mini, None).profile().expect("valid form");
  save_draft(&mut db, id, edited, now()).expect("draft from v1");
  publish(&mut db, id, now()).expect("publish v2");
  // Another editor's draft still based on v1.
  let high = form(&codex, &listed, Some("high")).profile().expect("valid form");
  db.save_draft(id, &Draft::new(high, Some(version(1)), now()))
    .expect("stale draft");

  let entry = ProjectAgents::load(&db)
    .expect("load")
    .profile(id)
    .cloned()
    .expect("profile");
  assert_eq!(
    entry.state,
    VersionState::StaleDraft {
      based_on: Some(version(1)),
      published: Some(version(2))
    }
  );
  assert!(matches!(
    publish(&mut db, id, now()),
    Err(StoreError::Publish(PublishError::StaleDraft { .. }))
  ));
  keep_draft_over_latest(&mut db, id, now()).expect("keep the draft");
  assert_eq!(publish(&mut db, id, now()).expect("publish"), version(3));
  assert_eq!(db.resources(ResourceKind::AgentProfile).expect("list").len(), 1);
}

#[test]
fn profiles_with_the_same_role_get_distinct_slugs() {
  let (repo, mut db) = project_db();
  let codex = codex(&repo);
  let listed = ModelSelection::Listed(model("gpt-5.5"));
  let profile: AgentProfile = form(&codex, &listed, None).profile().expect("valid form");
  let first = create_profile(&mut db, &profile, now()).expect("first");
  let second = create_profile(&mut db, &profile, now()).expect("second");
  let slug = |id| {
    db.resource(id)
      .expect("read")
      .expect("exists")
      .slug()
      .as_str()
      .to_owned()
  };
  assert_eq!(
    (slug(first), slug(second)),
    ("builder".to_owned(), "builder-2".to_owned())
  );
}
