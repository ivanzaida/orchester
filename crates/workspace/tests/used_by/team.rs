//! A team project asks its server (here the stand-in) and gets the same shape a local project computes. The stand-in
//! builds no execution bundle, so the tests name the versions a run pins, as its own tests do.

use std::path::Path;

use orchester_api::{
  Credential, ResourceCreate, SharedResourceKind, UsedBy, VersionContent,
  client::{Client, PlainHttp, ServerAddress},
  test_support::team_server::{CODE, Person, PinnedRun, RunReport, Standing, TeamServer},
};
use orchester_core::{
  id::{IdempotencyKey, ProjectId, ResourceId, RunId, ServerId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{
    CommandAction, CommandArg, CommandProgram, Pipeline, PromptText, Step, StepKey, StepKind, Tool, VersionNumber,
  },
  secret::SecretValue,
  sharing::MemberRole,
  time::Timestamp,
};
use orchester_store::{ProjectDb, SharingLink};
use orchester_workspace::{
  team_reads::TeamSession,
  used_by::{UsedByError, used_by},
};
use serde_json::Value;
use tempfile::TempDir;
use tokio::{runtime::Runtime, sync::watch};

use super::{repository, shown, tool_users};

/// A checkout linked to the stand-in's project, and the session of a member who joined it.
struct Team {
  server: TeamServer,
  dir: TempDir,
  runtime: Runtime,
  client: Client,
  token: String,
  linked: (ServerId, ProjectId),
  generation: watch::Sender<u64>,
}

fn client_at(url: &str) -> Client {
  Client::new(ServerAddress::parse(url).expect("address"), PlainHttp::NotAllowed).expect("client")
}

impl Team {
  fn new() -> Self {
    let server = TeamServer::start();
    server.share(&[("Kontur", "kontur", None)]);
    server.stand_as(Standing::Admin);
    let runtime = Runtime::new().expect("runtime");
    let client = client_at(&server.url());
    let joined = runtime
      .block_on(client.join_with_code(&orchester_api::CodeJoin {
        code: Credential::new(CODE.to_owned()),
        device: "used-by test".to_owned(),
        username: Some("dana".to_owned()),
        display_name: Some("Dana Ruiz".to_owned()),
        kind: Default::default(),
        identity_token: None,
        key_set: None,
      }))
      .expect("joined");
    let token = joined.token.expose().to_owned();
    let linked = (
      ServerId::generate(),
      server.project("kontur").expect("the shared project"),
    );
    let dir = repository();
    let key = ProjectKey::new("KON").expect("key");
    let project = Project::new(ProjectId::generate(), key.clone(), name("Kontur"));
    let mut db = ProjectDb::create(dir.path(), project).expect("project");
    let link = SharingLink {
      server: linked.0,
      project: linked.1,
      key,
      role: Some(MemberRole::Member),
      joined_at: Timestamp::from_unix_millis(100),
      cursor: 1,
      last_synced_at: Timestamp::from_unix_millis(100),
      removed_at: None,
      address: None,
    };
    db.start_sharing(&link, &[], (&[], &Default::default()))
      .expect("linked");
    Self {
      server,
      dir,
      runtime,
      client,
      token,
      linked,
      generation: watch::channel(0).0,
    }
  }

  fn token(&self) -> SecretValue {
    SecretValue::new(self.token.clone()).expect("token")
  }

  fn session(&self) -> TeamSession {
    TeamSession::new(
      (self.client.clone(), self.token()),
      self.linked,
      self.generation.subscribe(),
    )
  }

  fn create(&self, kind: SharedResourceKind, (slug, label): (&str, &str), body: Value) -> ResourceId {
    let id = ResourceId::generate();
    let create = ResourceCreate {
      idempotency_key: IdempotencyKey::generate(),
      id,
      kind,
      slug: slug.to_owned(),
      name: label.to_owned(),
      version: VersionContent {
        body,
        references: Vec::new(),
      },
    };
    self
      .runtime
      .block_on(self.client.create_resource(&self.token(), self.linked.1, &create))
      .expect("created");
    id
  }

  /// A run of `work` of the task `key`, last reported `progress`.
  fn pin(&self, (key, work): (&str, u32), holder: Option<Person>, pins: &[ResourceId]) -> RunId {
    let run = RunId::generate();
    self.server.pin_run(
      "kontur",
      PinnedRun {
        task_key: key.to_owned(),
        run,
        work,
        holder,
        last_report: RunReport::Progress,
        pins: pins.iter().map(|id| (*id, VersionNumber::FIRST)).collect(),
      },
    );
    run
  }

  fn used_by(&self, session: Option<&TeamSession>, resource: ResourceId) -> Result<UsedBy, UsedByError> {
    self.runtime.block_on(used_by(self.dir.path(), session, resource))
  }
}

fn name(value: &str) -> Name {
  Name::new(value).expect("name")
}

fn tool_body() -> Value {
  let action = CommandAction::new(
    CommandProgram::new("git").expect("program"),
    vec![CommandArg::new("status").expect("arg")],
  );
  serde_json::to_value(Tool::command("", action)).expect("tool body")
}

fn pipeline_body(tool: ResourceId) -> Value {
  let step = Step {
    key: StepKey::new("check").expect("key"),
    name: name("Check"),
    kind: StepKind::Tool { tool: Some(tool) },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  serde_json::to_value(Pipeline::new(vec![step]).expect("pipeline")).expect("pipeline body")
}

#[test]
fn used_by_team_asks_the_server_and_gives_the_shape_a_local_project_gives() {
  let team = Team::new();
  let tool = team.create(SharedResourceKind::Tool, ("deploy", "Deploy"), tool_body());
  for (slug, label) in [("delivery", "Delivery"), ("alpha", "alpha flow")] {
    team.create(SharedResourceKind::Pipeline, (slug, label), pipeline_body(tool));
  }
  team.pin(("KON-12", 1), Some(Person::Named("Dana Ruiz")), &[tool]);
  // KON-3 was taken over: work 1 is fenced and left out, work 2 is the current work.
  team.pin(("KON-3", 1), Some(Person::Named("Dana Ruiz")), &[tool]);
  team.pin(("KON-3", 2), Some(Person::Named("Lena Moroz")), &[tool]);
  team.pin(("KON-9", 1), None, &[ResourceId::generate()]);
  team.server.stand_as(Standing::Member);

  let session = team.session();
  let used = team.used_by(Some(&session), tool).expect("the tool's users");

  let (resources, _) = tool_users();
  assert_eq!(
    shown(&used),
    (resources, vec!["KON-3", "KON-12"]),
    "the local project's resources; the team server lists only the current work of each task"
  );
  let holders: Vec<_> = used
    .runs
    .iter()
    .map(|run| run.holder.as_ref().map(|holder| holder.display_name.as_str()))
    .collect();
  assert_eq!(holders, [Some("Lena Moroz"), Some("Dana Ruiz")]);
}

#[test]
fn used_by_team_gives_empty_lists_for_a_resource_nothing_uses_and_not_found_for_one_it_lacks() {
  let team = Team::new();
  let tool = team.create(SharedResourceKind::Tool, ("deploy", "Deploy"), tool_body());
  let session = team.session();

  let used = team.used_by(Some(&session), tool).expect("the tool's users");
  assert_eq!(shown(&used), (Vec::new(), Vec::new()));
  assert!(matches!(
    team.used_by(Some(&session), ResourceId::generate()),
    Err(UsedByError::NotFound)
  ));
}

#[test]
fn used_by_team_is_not_reached_instead_of_empty_when_the_server_cannot_be_reached() {
  let team = Team::new();
  let tool = team.create(SharedResourceKind::Tool, ("deploy", "Deploy"), tool_body());
  let port = std::net::TcpListener::bind(("127.0.0.1", 0))
    .expect("listen")
    .local_addr()
    .expect("address")
    .port();
  let closed = client_at(&format!("http://127.0.0.1:{port}"));
  let session = TeamSession::new((closed, team.token()), team.linked, team.generation.subscribe());

  let error = team
    .used_by(Some(&session), tool)
    .expect_err("an unreachable server has no answer");
  assert!(error.is_not_reached(), "{error:?}");
}

#[test]
fn used_by_team_refuses_a_session_that_is_missing_of_another_project_or_ended() {
  let team = Team::new();
  let tool = team.create(SharedResourceKind::Tool, ("deploy", "Deploy"), tool_body());

  assert!(matches!(team.used_by(None, tool), Err(UsedByError::NoTeamSession)));
  let other = TeamSession::new(
    (team.client.clone(), team.token()),
    (team.linked.0, ProjectId::generate()),
    team.generation.subscribe(),
  );
  assert!(matches!(
    team.used_by(Some(&other), tool),
    Err(UsedByError::OtherProject)
  ));
  let session = team.session();
  team.generation.send(1).expect("a generation");
  assert!(matches!(
    team.used_by(Some(&session), tool),
    Err(UsedByError::SessionChanged)
  ));
}

/// A checkout linked to no team reads locally and needs no session.
#[test]
fn used_by_local_project_needs_no_session() {
  let dir = repository();
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("KON").expect("key"),
    name("Kontur"),
  );
  ProjectDb::create(Path::new(dir.path()), project).expect("project");
  let runtime = Runtime::new().expect("runtime");
  let error = runtime
    .block_on(used_by(dir.path(), None, ResourceId::generate()))
    .expect_err("a local project without the resource");
  assert!(matches!(error, UsedByError::NotFound), "{error:?}");
}
