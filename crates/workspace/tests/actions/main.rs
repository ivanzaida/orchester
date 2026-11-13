//! The workspace's action dispatcher (`orchester_workspace::actions`, team authority P2) against a fake team server on
//! loopback with the team server's routes and shapes ([`server`], [`commands`]): a command's lost response is looked
//! up and never commits twice, also after a restart; an operation id reused with other inputs is refused here and by
//! the server; an operation never goes to another server project than the one it was recorded for; a work start's
//! `start` stays pending while a claim allows nothing locally; a receipt whose identity cannot be recorded fails its
//! local effect only; definition edits go to their own routes, are read again after a lost response, come with the
//! current value after a revision conflict and are refused for a member ([`definitions`]); a team task names only a
//! team pipeline, by its id ([`pipelines`]); and a local project's actions run as they always did ([`organize`]).
//!
//! `cargo test -p orchester-workspace --test actions actions_`

mod commands;
mod definitions;
#[path = "../team_work/fixture.rs"]
mod fixture;
mod http;
mod lifecycle;
mod organize;
mod pipelines;
mod run;
mod server;

use std::{fs, path::PathBuf, sync::Arc};

use orchester_api::{
  client::{Client, PlainHttp, ServerAddress},
  v2::{AuthorityErrorCode, AuthorityRef, LocalEffect, PolicyRevision, Subject, TeamCommandValue},
};
use orchester_core::{
  id::{IdempotencyKey, ModuleId, ProjectId, ResourceId, ServerId, TaskId},
  name::Name,
  project::{Project, ProjectKey},
  run::CommitId,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  sharing::TaskRevision,
  store::TaskStore,
  task::{Task, TaskDetails, TaskLimits, TaskUpdate, identity::TaskOwner, organization::TaskOrganization},
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb, StoreError, TeamOperationState};
use orchester_workspace::{
  actions::{
    Accepted, Action, ActionClass, ActionError, ActionOutcome, ClaimAction, Dispatcher, Intent, LocalResult,
    PolicyAction, Receipt, Refusal, TaskAction, TeamOutcome, WorkAction,
  },
  secrets::Vault,
  sharing::Servers,
  team_config::{TeamConfig, TeamResource, TeamVersion},
  team_reads::TeamSession,
  workspace::Workspace,
};
use server::{ADMIN, MEMBER, TeamServer};
use tempfile::TempDir;
use tokio::sync::watch;

/// No secret is read or kept here.
struct NoVault;

impl SecretVault for NoVault {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Err(VaultError::Failed("no secrets in these tests".to_owned()))
  }

  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::NotFound)
  }

  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Ok(())
  }
}

/// A project in a folder of its own, with the application database its writer needs.
struct Checkout {
  dir: TempDir,
  servers: Servers,
  /// The workspace `servers` read the team's configuration into, which a test fills as a stream's read would.
  workspace: Workspace,
  _data: TempDir,
}

impl Checkout {
  fn local() -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/actions-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    let dir = tempfile::Builder::new()
      .prefix("project-")
      .tempdir_in(&scratch)
      .expect("project");
    fs::create_dir(dir.path().join(".git")).expect("create .git");
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("KON").expect("key"),
      name("Kontur"),
    );
    ProjectDb::create(dir.path(), project).expect("create project");
    let data = tempfile::Builder::new()
      .prefix("data-")
      .tempdir_in(&scratch)
      .expect("data");
    let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
    let version = orchester_api::releases::ReleaseVersion::parse("0.5.0").expect("a release version");
    let servers = Servers::new(workspace.clone(), Vault::with(Arc::new(NoVault)), version);
    Self {
      dir,
      servers,
      workspace,
      _data: data,
    }
  }

  /// A project linked to `server`'s project.
  fn linked(server: &TeamServer) -> Self {
    Self::local().link(server)
  }

  /// The project linked to `server`'s project now.
  fn link(self, server: &TeamServer) -> Self {
    let connection = rusqlite::Connection::open(self.dir.path().join(".orchester/orchester.db")).expect("open");
    connection
      .execute(
        "INSERT INTO sharing (only_row, server_id, project_id, project_key, role, joined_at, event_cursor, \
         last_synced_at) VALUES (1, ?1, ?2, 'KON', 'member', 1, 0, 1)",
        [server.server.to_string(), server.project.to_string()],
      )
      .expect("link the project");
    self
  }

  /// The project made local again, as Make local does: its link is gone, its recorded operations stay.
  fn unlink(self) -> Self {
    rusqlite::Connection::open(self.dir.path().join(".orchester/orchester.db"))
      .expect("open")
      .execute("DELETE FROM sharing", [])
      .expect("make the project local");
    self
  }

  /// The team's configuration as the stream would read it: `pipelines`, each with its latest version's body.
  fn read_team(&self, pipelines: &[(ResourceId, &str, serde_json::Value)]) {
    let resources = pipelines.iter().map(|(id, slug, body)| TeamResource {
      id: *id,
      kind: orchester_core::resource::ResourceKind::Pipeline,
      name: name(slug),
      slug: (*slug).to_owned(),
      archived: false,
      revision: revision(1),
      updated: orchester_workspace::team_config::Changed {
        by: "Alice Moreau".to_owned(),
        at: orchester_core::time::Timestamp::from_unix_millis(0),
      },
      latest: Some(TeamVersion {
        number: orchester_core::resource::VersionNumber::FIRST,
        body: body.clone(),
        published_by: "Dana".to_owned(),
        published_at: orchester_core::time::Timestamp::from_unix_millis(1),
      }),
    });
    self
      .workspace
      .keep_team_config_for_test(self.dir.path(), TeamConfig::new(resources));
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.dir.path()).expect("open")
  }

  fn local_dispatcher(&self) -> Dispatcher {
    Dispatcher::new(self.dir.path().to_path_buf(), self.servers.clone(), None)
  }

  /// The dispatcher of a session signed in to `server` with `token`, and the session's generation it keeps.
  fn team(&self, server: &TeamServer, token: &str) -> (Dispatcher, watch::Sender<u64>) {
    self.session(server, token, server.server)
  }

  /// The dispatcher of a session signed in with `token` to the server `id` names, with `server`'s address and project.
  fn session(&self, server: &TeamServer, token: &str, id: ServerId) -> (Dispatcher, watch::Sender<u64>) {
    let client = Client::new(
      ServerAddress::parse(&server.url).expect("address"),
      PlainHttp::NotAllowed,
    )
    .expect("client");
    let token = SecretValue::new(token.to_owned()).expect("token");
    let generation = watch::channel(0).0;
    let session = TeamSession::new((client, token), (id, server.project), generation.subscribe());
    let dispatcher = Dispatcher::new(self.dir.path().to_path_buf(), self.servers.clone(), Some(session));
    (dispatcher, generation)
  }
}

fn name(value: &str) -> Name {
  Name::new(value).expect("name")
}

fn details(title: &str) -> TaskDetails {
  TaskDetails {
    title: name(title),
    brief: "Sign in with a password.".to_owned(),
    acceptance_criteria: vec!["A person can sign in".to_owned()],
    pipeline: None,
    limits: TaskLimits::NONE,
  }
}

fn create(title: &str) -> Action {
  Action::Task(TaskAction::Create {
    details: details(title),
    organization: TaskOrganization::default(),
  })
}

fn retitle(task: TaskId, read: u64, from: &str, to: &str) -> Action {
  Action::Task(TaskAction::Edit {
    task,
    read: revision(read),
    base: details(from),
    edited: details(to),
  })
}

fn revision(value: u64) -> TaskRevision {
  TaskRevision::try_from(value).expect("revision")
}

fn policy(read: u64, automatic_merge: bool) -> Action {
  Action::Policy(PolicyAction {
    read: PolicyRevision::try_from(read).expect("revision"),
    automatic_merge,
  })
}

fn team(outcome: ActionOutcome) -> TeamOutcome {
  match outcome {
    ActionOutcome::Team(outcome) => outcome,
    ActionOutcome::Local(local) => panic!("a team project's action ran locally: {local:?}"),
  }
}

/// The receipt of a completed command.
fn receipt(outcome: TeamOutcome) -> Box<Receipt> {
  match outcome {
    TeamOutcome::Completed(Accepted::Command(receipt)) => receipt,
    outcome => panic!("not a completed command: {outcome:?}"),
  }
}

/// The task a completed command's receipt names.
fn completed_task(outcome: TeamOutcome) -> TaskId {
  let receipt = receipt(outcome);
  let TeamCommandValue::Task { task, .. } = receipt.result else {
    panic!("not a task: {receipt:?}");
  };
  task.id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_a_lost_response_is_looked_up_and_never_commits_twice() {
  let server = TeamServer::start();
  let checkout = Checkout::linked(&server);
  let (dispatcher, _generation) = checkout.team(&server, ADMIN);
  server.lose_next_response();
  let intent = Intent::new(create("Sign in"));
  let lost = team(dispatcher.dispatch(&intent).await.expect("dispatched"));
  assert!(
    matches!(lost, TeamOutcome::AcceptanceUnknown { operation } if operation == intent.operation()),
    "{lost:?}"
  );
  assert_eq!(dispatcher.open_operations().expect("open"), [intent.operation()]);
  assert_eq!(server.counts(), (1, 1), "the server committed it");

  // After a restart: the operation is looked up, not sent again.
  let (restarted, _generation) = checkout.team(&server, ADMIN);
  let task = completed_task(restarted.recover(intent.operation()).await.expect("recovered"));
  assert_eq!(server.counts(), (1, 1), "looked up, never sent twice");
  let identity = checkout.db().task_identity(task).expect("read").expect("recorded");
  assert!(matches!(identity.owner, TaskOwner::Team { task: team, .. } if team.task == task));
  assert!(restarted.open_operations().expect("open").is_empty());

  // The same intent again reads the kept receipt.
  let again = team(restarted.dispatch(&intent).await.expect("dispatched"));
  assert_eq!(completed_task(again), task);
  assert_eq!(server.counts(), (1, 1));
  let record = checkout
    .db()
    .team_operation(intent.operation())
    .expect("read")
    .expect("kept");
  assert_eq!(
    (record.state, record.kind.as_str()),
    (TeamOperationState::Completed, "task.create")
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_an_operation_id_reused_with_other_inputs_is_refused() {
  let server = TeamServer::start();
  let checkout = Checkout::linked(&server);
  let (dispatcher, _generation) = checkout.team(&server, ADMIN);
  let intent = Intent::new(create("Sign in"));
  completed_task(team(dispatcher.dispatch(&intent).await.expect("dispatched")));
  let other = Intent::with_operation(intent.operation(), create("Log in"));
  let refused = dispatcher.dispatch(&other).await;
  assert!(
    matches!(refused, Err(ActionError::OperationMismatch(operation)) if operation == intent.operation()),
    "{refused:?}"
  );
  assert_eq!(server.counts(), (1, 1), "refused before anything was sent");

  // Another computer of the same account, which never recorded the id: the server refuses it for good.
  let elsewhere = Checkout::linked(&server);
  let (dispatcher, _generation) = elsewhere.team(&server, ADMIN);
  let refused = team(dispatcher.dispatch(&other).await.expect("dispatched"));
  let TeamOutcome::Rejected {
    refusal: Refusal::Command(error),
    retryable,
    ..
  } = refused
  else {
    panic!("not refused: {refused:?}");
  };
  assert_eq!(
    (error.code(), retryable),
    (AuthorityErrorCode::OperationMismatch, false)
  );
  assert_eq!(server.counts(), (2, 1), "no second task");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_an_operation_never_goes_to_another_server_project() {
  let first = TeamServer::start();
  let checkout = Checkout::linked(&first);
  let (dispatcher, _generation) = checkout.team(&first, ADMIN);
  first.lose_next_response();
  let intent = Intent::new(create("Sign in"));
  let lost = team(dispatcher.dispatch(&intent).await.expect("dispatched"));
  assert!(matches!(lost, TeamOutcome::AcceptanceUnknown { .. }), "{lost:?}");

  // Made local, then joined to another server project: the open operation is the first project's.
  let second = TeamServer::start();
  let checkout = checkout.unlink().link(&second);
  let (dispatcher, _generation) = checkout.team(&second, ADMIN);
  assert!(dispatcher.open_operations().expect("open").is_empty());
  let recovered = dispatcher.recover(intent.operation()).await.map(ActionOutcome::Team);
  for refused in [recovered, dispatcher.dispatch(&intent).await] {
    assert!(
      matches!(refused, Err(ActionError::OperationOfOtherProject(operation)) if operation == intent.operation()),
      "{refused:?}"
    );
  }
  assert_eq!(second.counts(), (0, 0), "nothing sent to the other project");
  let record = checkout
    .db()
    .team_operation(intent.operation())
    .expect("read")
    .expect("kept");
  assert_eq!(
    (record.state, record.project),
    (TeamOperationState::Sent, first.project)
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_a_work_start_stays_pending_and_classes_follow_the_owner() {
  let server = TeamServer::start();
  let task = server.add_task("Sign in");
  let checkout = Checkout::linked(&server);
  let (dispatcher, _generation) = checkout.team(&server, MEMBER);
  let subject = Subject::Task { id: task.id };
  let acquire = Action::Claim(ClaimAction::Acquire { subject });
  let edit = retitle(task.id, 1, "Sign in", "Log in");
  assert_eq!(dispatcher.classify(&acquire).expect("class"), ActionClass::ServerOnly);
  assert_eq!(dispatcher.classify(&edit).expect("class"), ActionClass::ServerOnly);

  let claimed = receipt(team(
    dispatcher.dispatch(&Intent::new(acquire)).await.expect("dispatched"),
  ));
  let TeamCommandValue::Claim { claim, effects, .. } = claimed.result else {
    panic!("not a claim: {claimed:?}");
  };
  assert!(effects.is_empty(), "a claim allows nothing here");
  let start = Action::Work(WorkAction {
    authority: AuthorityRef {
      subject,
      claim_id: claim.id,
      work_id: None,
    },
    base_commit: CommitId::new(&"b".repeat(40)).expect("commit"),
    read: revision(1),
    module: Some((ModuleId::generate(), revision(1))),
  });
  assert_eq!(
    dispatcher.classify(&start).expect("class"),
    ActionClass::ServerThenLocalExecution
  );
  let intent = Intent::new(start);
  let pending = team(dispatcher.dispatch(&intent).await.expect("dispatched"));
  let TeamOutcome::AcceptedPending { pending, .. } = pending else {
    panic!("not pending: {pending:?}");
  };
  assert!(matches!(pending.as_slice(), [LocalEffect::Start { .. }]), "{pending:?}");
  assert_eq!(dispatcher.open_operations().expect("open"), [intent.operation()]);

  let unsigned = Dispatcher::new(checkout.dir.path().to_path_buf(), checkout.servers.clone(), None);
  let refused = unsigned.dispatch(&Intent::new(edit.clone())).await;
  assert!(matches!(refused, Err(ActionError::NoTeamSession)), "{refused:?}");
  let (elsewhere, _generation) = checkout.session(&server, ADMIN, ServerId::generate());
  let refused = elsewhere.dispatch(&Intent::new(edit)).await;
  assert!(
    matches!(refused, Err(ActionError::OtherProject)),
    "a session of another server with the same project id: {refused:?}"
  );
  assert_eq!(server.writes(), 0);
  assert!(
    checkout.db().tasks().expect("tasks").is_empty(),
    "never a local change instead"
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_a_receipt_naming_a_local_task_id_fails_its_local_effect_only() {
  let server = TeamServer::start();
  let checkout = Checkout::local();
  let local = checkout
    .db()
    .create_tasks(IdempotencyKey::generate(), vec![details("Kept here")], Timestamp::now())
    .expect("a local task")
    .remove(0);
  let checkout = checkout.link(&server);
  server.next_task_id(local.id());
  let (dispatcher, _generation) = checkout.team(&server, ADMIN);
  let intent = Intent::new(create("Sign in"));
  let failed = team(dispatcher.dispatch(&intent).await.expect("dispatched"));
  let TeamOutcome::LocalEffectFailed { receipt, error } = failed else {
    panic!("not a failed local effect: {failed:?}");
  };
  assert!(
    matches!(error, StoreError::TaskIdTaken(id) if id == local.id()),
    "{error:?}"
  );
  assert_eq!(receipt.operation_id, intent.operation(), "still committed");
  let record = checkout
    .db()
    .team_operation(intent.operation())
    .expect("read")
    .expect("kept");
  assert_eq!(record.state, TeamOperationState::EffectFailed);
  assert_eq!(dispatcher.open_operations().expect("open"), [intent.operation()]);
  assert_eq!(server.counts(), (1, 1));
}

/// The task a local action saved.
fn saved_locally(outcome: ActionOutcome) -> Task {
  let ActionOutcome::Local(LocalResult::Task(update)) = outcome else {
    panic!("not a local task: {outcome:?}");
  };
  match *update {
    TaskUpdate::Saved(task) => task,
    TaskUpdate::Conflict { current } => panic!("a conflict with {current:?}"),
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_local_actions_run_as_before() {
  let checkout = Checkout::local();
  let dispatcher = checkout.local_dispatcher();
  let intent = Intent::new(create("Sign in"));
  assert_eq!(
    dispatcher.classify(intent.action()).expect("class"),
    ActionClass::LocalOnly
  );
  let saved = saved_locally(dispatcher.dispatch(&intent).await.expect("dispatched"));
  let again = saved_locally(dispatcher.dispatch(&intent).await.expect("dispatched"));
  assert_eq!(again.id(), saved.id(), "the same intent saves one task");
  let read = checkout
    .db()
    .task(saved.id())
    .expect("read")
    .expect("the task")
    .revision();
  let edit = Intent::new(retitle(saved.id(), read.get(), "Sign in", "Log in"));
  let edited = saved_locally(dispatcher.dispatch(&edit).await.expect("dispatched"));
  assert_eq!(edited.details().title.as_str(), "Log in");
  let tasks = checkout.db().tasks().expect("tasks");
  assert_eq!(tasks.len(), 1);
  assert_eq!(tasks[0].details().title.as_str(), "Log in");

  let policy = policy(1, true);
  assert!(matches!(dispatcher.classify(&policy), Err(ActionError::TeamOnly)));
  let refused = dispatcher.dispatch(&Intent::new(policy)).await;
  assert!(matches!(refused, Err(ActionError::TeamOnly)), "{refused:?}");
  assert!(dispatcher.open_operations().expect("open").is_empty());
}
