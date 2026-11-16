//! The team's configuration through the stream (`docs/design/team-configuration-live.md`): resources the server has
//! and versions published later are read into memory from the fake server, as sent, with no binding made of them and
//! nothing written to the project's database.

use std::{collections::BTreeSet, path::Path};

use orchester_api::{ReferenceTarget, SharedResourceKind};
use orchester_core::{
  connection::{AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn},
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::{AgentPermissions, StepPermissions},
  resource::{
    ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, ResourceKind, Step, StepKey, StepKind,
    TeamAgentProfile, VersionNumber,
  },
  secret::SecretName,
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::Value;

use super::{ALICE, Member, SETTLED, TeamServer, Workspace, seen_within};

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

pub(super) fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("valid key")
}

fn at() -> Timestamp {
  Timestamp::from_unix_millis(1)
}

/// A signed-in Codex connection named `called`, kept in `db`.
pub(super) fn codex(db: &mut ProjectDb, root: &Path, called: &str) -> ConnectionId {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    name(called),
    AgentProvider::CodexCli,
    root.join("codex.exe"),
    at(),
  )
  .expect("absolute path");
  connection
    .record_check(ConnectionCheck {
      at: at(),
      result: CheckResult::Available {
        version: "0.139.0".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
      },
    })
    .expect("an agent check fits a CLI connection");
  let connection = connection.into_connection();
  db.create_connection(&connection).expect("kept");
  connection.id()
}

/// The team's Builder profile: it runs with Codex, uses the team MCP servers `servers` and has `permissions`.
pub(super) fn team_profile(servers: &[ResourceId], permissions: AgentPermissions) -> TeamAgentProfile {
  TeamAgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    (
      AgentProvider::CodexCli,
      ModelId::new("gpt-5.5").expect("valid model"),
      ProviderSettings::default(),
    ),
  )
  .with_permissions(permissions)
  .and_then(|profile| profile.with_mcp_servers(servers.to_vec()))
  .expect("no writable folders, distinct servers")
}

/// [`team_profile`] with a profile's first permissions, as the server keeps it.
pub(super) fn team_builder(servers: &[ResourceId]) -> Value {
  serde_json::to_value(team_profile(servers, AgentPermissions::before_permissions())).expect("a profile serializes")
}

/// The references of [`team_builder`].
pub(super) fn team_references(servers: &[ResourceId]) -> Vec<ReferenceTarget> {
  ReferenceTarget::of_profile(&team_profile(servers, AgentPermissions::before_permissions()))
}

pub(super) fn step(step: &str, kind: StepKind) -> Step {
  Step {
    key: key(step),
    name: name(step),
    kind,
    instructions: PromptText::new("").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

pub(super) fn agent(profile: ResourceId) -> StepKind {
  StepKind::Agent {
    profile: Some(profile),
    repository: RepositoryAccess::Write,
    model: None,
    permissions: StepPermissions::default(),
  }
}

pub(super) fn pipeline(steps: Vec<Step>) -> Value {
  serde_json::to_value(Pipeline::new(steps).expect("pipeline")).expect("a pipeline serializes")
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_profile_published_on_the_server_is_cached_with_its_references_and_binds_nothing() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let (profile, docs, delivery) = (ResourceId::generate(), ResourceId::generate(), ResourceId::generate());
  let team = team_profile(&[docs], AgentPermissions::before_permissions())
    .with_secrets(vec![SecretName::new("DOCS_TOKEN").expect("a secret name")])
    .expect("one secret");
  let references = ReferenceTarget::of_profile(&team);
  let body = serde_json::to_value(&team).expect("a profile serializes");
  server.publish(
    (profile, SharedResourceKind::AgentProfile, "Builder"),
    body.clone(),
    references.clone(),
  );
  server.publish(
    (delivery, SharedResourceKind::Pipeline, "Delivery"),
    pipeline(vec![step("implement", agent(profile))]),
    Vec::new(),
  );
  let workspace = Workspace::linked(&server);
  codex(&mut workspace.db(), &workspace.root, "Laptop");
  let member = Member::of(&server, ALICE);
  let configs = member.configs.clone();
  let stream = workspace.follow(member);
  let read = |resource| {
    let config = configs.read(&workspace.root).config().cloned()?;
    let found = config.resource(resource)?;
    Some((
      found.number()?,
      found.latest.clone()?.body,
      found.latest.clone()?.published_by,
    ))
  };
  seen_within(SETTLED, "the snapshot's resources", || {
    read(delivery).is_some() && read(profile).is_some()
  })
  .await;

  let (number, kept, by) = read(profile).expect("read");
  assert_eq!(number, VersionNumber::FIRST);
  assert_eq!(kept, body, "the server's body as published");
  assert_eq!(by, "Alice Moreau");

  server.publish((profile, SharedResourceKind::AgentProfile, "Builder"), body, references);
  seen_within(SETTLED, "v2 from its event", || {
    read(profile).is_some_and(|(number, ..)| number.get() == 2)
  })
  .await;
  seen_within(SETTLED, "the cursor at the last event", || {
    workspace.link().cursor == server.cursor()
  })
  .await;
  let db = workspace.db();
  assert!(
    db.resources(ResourceKind::AgentProfile).expect("read").is_empty()
      && db.resources(ResourceKind::Pipeline).expect("read").is_empty(),
    "nothing of the team's is written to the project's database, and nothing is bound"
  );
  stream.abort();
}
