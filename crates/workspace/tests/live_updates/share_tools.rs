//! A tool goes to the team in the team's form (`docs/design/team-configuration.md`, "Who owns what"): the secret it
//! sends, held by an id of this computer, goes by its name, and the references name it too. Share uploads a local tool
//! that way, every version, an archived secret by the name it had; one whose secret the project never had is refused;
//! an admin's Save publishes again by name a team tool whose version named its secret by id.
//!
//! `cargo test -p orchester-workspace --test live_updates live_updates_share_tools_`

use std::collections::BTreeMap;

use orchester_api::{ReferenceTarget, SharedResourceKind};
use orchester_core::{
  id::{ProjectId, ResourceId, SecretId},
  name::Name,
  resource::{
    AccessPolicy, AccessWindow, ApprovalMode, DeniedBehavior, Draft, HttpAuth, HttpMethod, HttpRequest, ResourceKind,
    Tool, ToolAction,
  },
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName, archive},
  sharing::{MemberRole, TaskRevision},
  store::{ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  agents::team_body_edit,
  sharing::{ShareRefusalReason, SharingError, sync::upload_configuration, team_edit::EditRefused},
  team_config::{TeamConfig, TeamResource, TeamVersion},
};
use serde_json::{Value, json};

use super::{
  ALICE, TeamServer,
  desktop::Desktop,
  publish::{Publisher, number},
  share::{Sharer, name, slug},
  share_team_form::MemoryVault,
  support::{Fixture, at},
};

fn sentry(secret: SecretId) -> Tool {
  let request = HttpRequest::new(
    HttpMethod::Get,
    "https://sentry.example.test/api/issues/".into(),
    "sentry.example.test".into(),
    HttpAuth::Bearer { secret },
    BTreeMap::new(),
    BTreeMap::new(),
  )
  .expect("a request");
  let access = AccessPolicy::new(
    BTreeMap::new(),
    Name::new("Production").expect("name"),
    ApprovalMode::Never,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("access");
  Tool::new("Issues.", ToolAction::Http(request), Some(access)).expect("a tool")
}

/// A local secret `SENTRY_TOKEN` of the project in `db`, under `id`.
fn sentry_token(db: &mut ProjectDb, id: SecretId) {
  let new = NewSecret {
    name: SecretName::new("SENTRY_TOKEN").expect("name"),
    kind: SecretKind::AccessToken,
    environment: Environment::Production,
  };
  db.create_secret(&Secret::new(id, new, at(1))).expect("a local secret");
}

/// The body `local` goes to the team as: its secret by `secret`, its name.
fn by_name(local: &Tool, secret: &str) -> Value {
  let mut body = serde_json::to_value(local).expect("json");
  body["action"]["http"]["auth"]["bearer"]["secret"] = json!(secret);
  body
}

fn sentry_reference() -> Vec<ReferenceTarget> {
  vec![ReferenceTarget::SecretName {
    name: SecretName::new("SENTRY_TOKEN").expect("name"),
  }]
}

/// A published tool `sentry-issues` of Alice's project, `body` at v1.
fn tool_of(alice: &mut Sharer, body: &Tool) -> ResourceId {
  let db = &mut alice.fixture.db;
  let tool = db
    .create_resource(slug("sentry-issues"), name("sentry-issues"), body, at(30))
    .expect("a tool")
    .id();
  db.publish_draft::<Tool>(tool, at(31)).expect("v1");
  tool
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_tools_a_tool_goes_with_its_secret_by_name() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut alice = Sharer::new();
  let token = SecretId::generate();
  sentry_token(&mut alice.fixture.db, token);
  let local = sentry(token);
  let tool = tool_of(&mut alice, &local);

  alice.share(&server).await;

  let versions = server.versions_of(tool);
  assert_eq!(versions.len(), 1);
  assert_eq!(versions[0].body, by_name(&local, "SENTRY_TOKEN"));
  assert!(
    !versions[0].body.to_string().contains(&token.to_string()),
    "the id stays on this computer"
  );
  assert_eq!(versions[0].references, sentry_reference());
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_tools_a_version_whose_secret_the_project_never_had_is_refused_old_or_new() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut alice = Sharer::new();
  let (token, gone) = (SecretId::generate(), SecretId::generate());
  sentry_token(&mut alice.fixture.db, token);
  let tool = tool_of(&mut alice, &sentry(gone));
  let publisher = Publisher::online(&server, ALICE);

  let refused = upload_configuration(&publisher, alice.fixture.repo.path()).await;
  let Err(SharingError::ShareRefused { refusals }) = &refused else {
    panic!("{refused:?}");
  };
  assert_eq!(refusals.len(), 1, "{refusals:?}");
  assert_eq!(
    (refusals[0].resource, &refusals[0].reason),
    (tool, &ShareRefusalReason::ToolSecretGone)
  );
  assert!(server.versions_of(tool).is_empty(), "nothing was sent");

  let db = &mut alice.fixture.db;
  db.save_draft(tool, &Draft::new(sentry(token), Some(number(1)), at(32)))
    .expect("a draft of v2");
  db.publish_draft::<Tool>(tool, at(33)).expect("v2");
  let still = upload_configuration(&publisher, alice.fixture.repo.path()).await;
  assert!(
    matches!(&still, Err(SharingError::ShareRefused { refusals }) if refusals.len() == 1),
    "v1 names a secret the project never had, and no id goes to the team: {still:?}"
  );
  assert!(server.versions_of(tool).is_empty(), "nothing was sent");
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_tools_an_archived_secret_goes_by_the_name_it_had() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut alice = Sharer::new();
  let token = SecretId::generate();
  sentry_token(&mut alice.fixture.db, token);
  let tool = tool_of(&mut alice, &sentry(token));
  archive(
    &mut alice.fixture.db,
    &MemoryVault::default(),
    ProjectId::generate(),
    token,
    at(34),
  )
  .expect("archived");

  alice.share(&server).await;

  let versions = server.versions_of(tool);
  assert_eq!(versions.len(), 1);
  assert_eq!(versions[0].body, by_name(&sentry(token), "SENTRY_TOKEN"));
  assert_eq!(versions[0].references, sentry_reference());
}

/// Alice's desktop, an admin, holding the team tool at v1 with `local`'s body, as a read of the server would.
fn holding(server: &TeamServer, local: &Tool) -> (Desktop, ResourceId) {
  let id = ResourceId::generate();
  let body = serde_json::to_value(local).expect("json");
  server.publish(
    (id, SharedResourceKind::Tool, "sentry-issues"),
    body.clone(),
    Vec::new(),
  );
  let desktop = Desktop::signed_in(server, ALICE, Fixture::new(), &[]);
  ProjectDb::open(desktop.root())
    .expect("the project")
    .record_sharing_state(Some(MemberRole::Owner), None, None)
    .expect("the role");
  let resource = TeamResource {
    id,
    kind: ResourceKind::Tool,
    name: Name::new("sentry-issues").expect("name"),
    archived: false,
    revision: TaskRevision::try_from(1).expect("a revision"),
    updated: orchester_workspace::team_config::Changed {
      by: "Alice Moreau".to_owned(),
      at: orchester_core::time::Timestamp::from_unix_millis(0),
    },
    latest: Some(TeamVersion {
      number: number(1),
      body,
      published_by: "Alice Moreau".to_owned(),
      published_at: Timestamp::from_unix_millis(1),
    }),
  };
  desktop
    .workspace
    .keep_team_config_for_test(desktop.root(), TeamConfig::new([resource]));
  (desktop, id)
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_share_tools_an_admins_save_publishes_a_tool_named_by_id_again_by_name() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  server.make_admin(ALICE);
  let token = SecretId::generate();
  let old = sentry(token);
  let (desktop, id) = holding(&server, &old);

  let refused = team_body_edit::save_tool(&desktop.servers, desktop.root(), (id, number(1)), &old)
    .await
    .expect_err("this project has no secret of that id");
  assert!(matches!(refused, EditRefused::Failed(_)), "{refused:?}");
  assert_eq!(server.versions_of(id).len(), 1, "nothing was published");

  let mut db = ProjectDb::open(desktop.root()).expect("the project");
  sentry_token(&mut db, token);
  let saved = team_body_edit::save_tool(&desktop.servers, desktop.root(), (id, number(1)), &old)
    .await
    .expect("published again by name");

  assert_eq!(saved, number(2));
  let versions = server.versions_of(id);
  assert_eq!(versions[1].body, by_name(&old, "SENTRY_TOKEN"));
  assert_eq!(versions[1].references, sentry_reference());
  assert!(
    ResourceStore::resources(&db, ResourceKind::Tool)
      .expect("read")
      .is_empty(),
    "nothing is kept on this computer"
  );
}
