//! A shared project's live event stream against a fake team server on loopback (`docs/architecture/server.md`, "Live
//! updates", "Cursor and applying events"): two workspaces' caches see each other's task change, claim and run progress
//! within a second; a connect without a cursor applies the snapshot and a reconnect resumes from the cursor; a delta
//! the cache cannot take reads that task; `revoked` stops the stream and the list reads *not a member*; reconnects
//! wait 1 s doubling to 60 s with jitter and the project's state moves through *live*, *reconnecting*, *offline* and
//! *not a member* ([`reconnects`]), also as the desktop's servers keep it, and goes once the project is local. Without
//! the minute's poll, a run's workflow status the server could not take goes with the minute's round, without *Check
//! again*. The team's configuration comes with it ([`config`]), also to a stream resumed from a cursor that came
//! without the snapshot ([`joined`]), its admins publish it from here ([`publish`]), Share uploads it with its history
//! and Make local keeps it ([`share`]), and a resource archived there leaves this computer ([`archived`]). This
//! computer's catalog is reported while the stream is open ([`catalog`]).
//!
//! `cargo test -p orchester-workspace --test live_updates live_updates_`

mod archived;
mod catalog;
mod claim_ends;
mod config;
mod desktop;
mod http;
mod joined;
mod leaving;
mod mcp_servers;
mod merged;
mod publish;
mod reconnects;
mod server;
mod share;
mod share_fill_ins;
mod share_team_form;
#[path = "../support/mod.rs"]
mod support;
mod team_copy;
mod team_created;
mod team_pipeline_echo;
mod team_refused;
mod widening;

use std::{
  collections::HashMap,
  fs,
  path::PathBuf,
  sync::{Arc, Mutex},
  time::{Duration, Instant},
};

use desktop::Desktop;
use orchester_api::{
  CatalogReport, ClaimTake, RunReport, SharedResource, SharedTask, TaskChanges, TaskPatch, Versions,
  client::{Client, ClientError, EventStream, PlainHttp, ServerAddress, StreamEnd},
};
use orchester_core::{
  id::{ProjectId, ResourceId, ServerId, TaskId},
  name::Name,
  project::{Project, ProjectKey},
  resource::VersionNumber,
  run::TaskPhase,
  secret::SecretValue,
  sharing::{ClaimKind, MemberRole, TaskRevision},
  task::organization::WorkflowStatus,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_ipc::SessionEnd;
use orchester_store::{CachedTask, ProjectDb, SharingLink};
use orchester_workspace::{
  activity::Activity,
  messages::Message,
  runner::Runners,
  sharing::{
    SharingError,
    sync::{Clock, Source, Stopped, SyncState, SystemClock, follow},
  },
  team_config::TeamConfigs,
};
use server::TeamServer;
use support::Fixture;
use tempfile::TempDir;

const ALICE: &str = "alice-token";
const BOB: &str = "bob-token";
/// How soon a change made on one workspace must reach the other's cache.
const LIVE: Duration = Duration::from_secs(1);
/// How long a test waits for what a stream does when that promise is not what it checks: a full run of this binary
/// starts dozens of streams, fake servers and project databases at once, which can hold a first snapshot or a
/// reconnect past a second.
const SETTLED: Duration = Duration::from_secs(5);

/// A member's view of the project's server: its client signed in with the member's token, and the states its stream
/// reached.
struct Member {
  client: Client,
  token: SecretValue,
  project: ProjectId,
  states: Mutex<Vec<SyncState>>,
  /// The resources its stream saw archived, in order, with who archived each.
  archived: Mutex<Vec<(ResourceId, Option<String>)>>,
  /// The team's configuration its stream read, in memory.
  configs: TeamConfigs,
}

impl Member {
  fn of(server: &TeamServer, token: &str) -> Self {
    let address = ServerAddress::parse(&server.url).expect("address");
    Self {
      client: Client::new(address, PlainHttp::NotAllowed).expect("client"),
      token: SecretValue::new(token.to_owned()).expect("token"),
      project: server.project,
      states: Mutex::new(Vec::new()),
      archived: Mutex::new(Vec::new()),
      configs: TeamConfigs::default(),
    }
  }

  fn states(&self) -> Vec<SyncState> {
    self.states.lock().expect("states").clone()
  }

  /// Retitles `task`, read at `revision`.
  async fn retitle(&self, task: TaskId, revision: u64, title: &str) {
    let patch = TaskPatch {
      revision: TaskRevision::try_from(revision).expect("a revision"),
      changes: TaskChanges {
        title: Some(title.to_owned()),
        ..TaskChanges::default()
      },
    };
    let changed = self.client.update_task(&self.token, self.project, task, &patch).await;
    changed.expect("the change");
  }

  async fn take(&self, task: TaskId) {
    let take = ClaimTake { kind: ClaimKind::Run };
    let taken = self.client.claim(&self.token, self.project, task, &take).await;
    taken.expect("the claim");
  }

  async fn report(&self, task: TaskId, phase: TaskPhase, step: &str) {
    let report = RunReport {
      number: 1,
      phase,
      step: Some(step.to_owned()),
      started_at: 1_000,
      phase_since: 1_000,
    };
    let reported = self.client.report_run(&self.token, self.project, task, &report).await;
    reported.expect("the report");
  }
}

impl Source for Member {
  async fn open(&self, cursor: Option<u64>) -> Result<EventStream, StreamEnd> {
    self.client.events(&self.token, self.project, cursor).await
  }

  async fn task(&self, task: TaskId) -> Result<SharedTask, SharingError> {
    Ok(self.client.task(&self.token, self.project, task).await?)
  }

  async fn resources(&self) -> Result<Vec<SharedResource>, SharingError> {
    Ok(self.client.resources(&self.token, self.project).await?.resources)
  }

  async fn versions(&self, resource: ResourceId, after: Option<VersionNumber>) -> Result<Versions, SharingError> {
    Ok(self.client.versions(&self.token, self.project, resource, after).await?)
  }

  async fn report_catalog(&self, report: CatalogReport) -> Result<(), SharingError> {
    Ok(self.client.report_catalog(&self.token, self.project, &report).await?)
  }

  async fn archived(&self, resource: ResourceId, by: Option<&str>) {
    self
      .archived
      .lock()
      .expect("archived")
      .push((resource, by.map(str::to_owned)));
  }

  fn changed(&self) {}

  fn state(&self, state: SyncState) {
    self.states.lock().expect("states").push(state);
  }

  fn configs(&self) -> &TeamConfigs {
    &self.configs
  }

  fn unreached(&self, error: &SharingError) -> Message {
    error.message(self.client.address())
  }

  async fn retried(&self) {
    std::future::pending::<()>().await;
  }
}

/// A workspace's project linked to the server's project, its cache empty and without a cursor: the stream's snapshot
/// fills it.
struct Workspace {
  _repo: TempDir,
  root: PathBuf,
}

impl Workspace {
  fn linked(server: &TeamServer) -> Self {
    Self::linked_as(server, MemberRole::Member)
  }

  /// [`Self::linked`], the person's role in the project `role`.
  fn linked_as(server: &TeamServer, role: MemberRole) -> Self {
    let repo = tempfile::tempdir().expect("temporary directory");
    fs::create_dir(repo.path().join(".git")).expect("create .git");
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("ORC").expect("key"),
      Name::new("Orchester").expect("name"),
    );
    let mut db = ProjectDb::create(repo.path(), project).expect("the project database");
    link(&mut db, server, role);
    let root = repo.path().to_owned();
    Self { _repo: repo, root }
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.root).expect("the project database")
  }

  fn task(&self, task: TaskId) -> Option<CachedTask> {
    self.db().cached_task(task).expect("the cache")
  }

  fn link(&self) -> SharingLink {
    self.db().sharing().expect("the link").expect("still linked")
  }

  /// Follows the project's stream as `member`, on the system clock, until it stops.
  fn follow(&self, member: Member) -> tokio::task::JoinHandle<Stopped> {
    let root = self.root.clone();
    tokio::spawn(async move { follow(&member, &root, &SystemClock).await })
  }
}

/// Links the project of `db` to the server's project, its cache empty and without a cursor, the person's role there
/// `role`.
fn link(db: &mut ProjectDb, server: &TeamServer, role: MemberRole) {
  let link = SharingLink {
    server: ServerId::generate(),
    project: server.project,
    key: ProjectKey::new("ORC").expect("key"),
    role: Some(role),
    joined_at: Timestamp::from_unix_millis(1),
    cursor: 0,
    last_synced_at: Timestamp::from_unix_millis(1),
    removed_at: None,
    address: Some("https://team.example/orchester".to_owned()),
  };
  db.start_sharing(&link, &[], (&[], &HashMap::new())).expect("linked");
}

/// Waits until `seen` holds, at most `within`; `what` names it when it does not.
async fn seen_within(within: Duration, what: &str, mut seen: impl FnMut() -> bool) {
  let start = Instant::now();
  while !seen() {
    assert!(start.elapsed() < within, "{what}: not seen within {within:?}");
    tokio::time::sleep(Duration::from_millis(10)).await;
  }
}

/// Alice retitles the server's first task, still at revision 1, and this waits until `cached` reads the new title: the
/// stream then reads events again, which it does only after the reads it makes as it opens (the team's configuration,
/// and the resources of a stream resumed from a cursor). A session revoked during those reads ends signed out (`401`)
/// instead of with the stream's `revoked`.
async fn reading_events(server: &TeamServer, cached: impl Fn() -> Option<CachedTask>) {
  Member::of(server, ALICE)
    .retitle(server.task_ids()[0], 1, "Read after the reconnect")
    .await;
  seen_within(SETTLED, "a change after the reconnect", || {
    cached().is_some_and(|task| title(&task) == "Read after the reconnect")
  })
  .await;
}

fn title(task: &CachedTask) -> &str {
  task.title.as_str()
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_two_workspaces_see_each_others_task_change_claim_and_run_progress_within_a_second() {
  let server = TeamServer::start(
    &[(ALICE, "Alice Moreau"), (BOB, "Bob Lind")],
    &["Status bar", "Holder column"],
  );
  let [status, holder] = server.task_ids()[..] else {
    panic!("two tasks");
  };
  let (alice, bob) = (Workspace::linked(&server), Workspace::linked(&server));
  let streams = [
    alice.follow(Member::of(&server, ALICE)),
    bob.follow(Member::of(&server, BOB)),
  ];
  seen_within(SETTLED, "both snapshots", || {
    alice.task(holder).is_some() && bob.task(holder).is_some()
  })
  .await;

  Member::of(&server, ALICE).retitle(status, 1, "Status bar item").await;
  seen_within(LIVE, "Alice's change on Bob's", || {
    bob.task(status).is_some_and(|task| title(&task) == "Status bar item")
  })
  .await;
  let changed = bob.task(status).expect("cached");
  assert_eq!(
    (changed.revision.get(), changed.updated_by.as_str()),
    (2, "Alice Moreau")
  );
  seen_within(LIVE, "Alice's own change", || {
    alice.task(status).is_some_and(|task| task.revision.get() == 2)
  })
  .await;

  Member::of(&server, BOB).take(holder).await;
  seen_within(LIVE, "Bob's claim on Alice's", || {
    alice.task(holder).is_some_and(|task| task.claim.is_some())
  })
  .await;
  let claim = alice.task(holder).and_then(|task| task.claim).expect("the claim");
  assert_eq!((claim.holder.name.as_str(), claim.kind), ("Bob Lind", ClaimKind::Run));

  let bobs = Member::of(&server, BOB);
  bobs.report(holder, TaskPhase::Running, "Implement").await;
  seen_within(LIVE, "Bob's run on Alice's", || {
    alice.task(holder).is_some_and(|task| task.run.is_some())
  })
  .await;
  bobs.report(holder, TaskPhase::Waiting, "Review").await;
  seen_within(LIVE, "Bob's run progress on Alice's", || {
    alice
      .task(holder)
      .and_then(|task| task.run)
      .is_some_and(|run| run.phase == TaskPhase::Waiting && run.step.as_deref() == Some("Review"))
  })
  .await;
  assert_eq!(
    server.reads(),
    [holder, holder],
    "each cache read the task once, for the first report of a run it did not have"
  );
  seen_within(LIVE, "both cursors at the last event", || {
    alice.link().cursor == server.cursor() && bob.link().cursor == server.cursor()
  })
  .await;
  for stream in streams {
    stream.abort();
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_connect_without_a_cursor_applies_the_snapshot_and_a_reconnect_resumes_from_its_cursor() {
  let server = TeamServer::start(
    &[(ALICE, "Alice Moreau"), (BOB, "Bob Lind")],
    &["Status bar", "Holder column"],
  );
  let [status, holder] = server.task_ids()[..] else {
    panic!("two tasks");
  };
  Member::of(&server, BOB).retitle(status, 1, "Status bar item").await;
  let workspace = Workspace::linked(&server);
  let stream = workspace.follow(Member::of(&server, ALICE));
  seen_within(SETTLED, "the snapshot's cursor", || {
    workspace.link().cursor == server.cursor()
  })
  .await;
  let cached = workspace.db().cached_tasks().expect("the cache");
  let titles: Vec<_> = cached.iter().map(title).collect();
  assert_eq!(titles, ["Status bar item", "Holder column"], "the snapshot's tasks");
  let tags = workspace.db().project_tags().expect("the tags");
  let tag_names: Vec<_> = tags.iter().map(|tag| tag.name().as_str().to_owned()).collect();
  assert_eq!(tag_names, server.tag_names(), "the snapshot's tags");
  assert_eq!(server.connects(), [None], "the first connect has no cursor");

  let cursor = server.cursor();
  server.drop_streams();
  // The reconnect waits about a second, then resumes where the cache is.
  seen_within(Duration::from_secs(3), "the reconnect", || server.connects().len() == 2).await;
  assert_eq!(server.connects()[1], Some(cursor));
  Member::of(&server, BOB).retitle(holder, 1, "Holder").await;
  seen_within(SETTLED, "a change after the reconnect", || {
    workspace.task(holder).is_some_and(|task| title(&task) == "Holder")
  })
  .await;
  stream.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_stale_delta_reads_that_task_and_revoked_stops_the_stream() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau"), (BOB, "Bob Lind")], &["Status bar"]);
  let task = server.task_ids()[0];
  let workspace = Workspace::linked(&server);
  let stream = workspace.follow(Member::of(&server, ALICE));
  seen_within(SETTLED, "the snapshot", || workspace.task(task).is_some()).await;

  server.retitle_unannounced(task, "Missed");
  Member::of(&server, BOB).retitle(task, 2, "Status bar item").await;
  seen_within(SETTLED, "the task read again", || {
    workspace
      .task(task)
      .is_some_and(|cached| title(&cached) == "Status bar item")
  })
  .await;
  assert_eq!(
    server.reads(),
    [task],
    "made on a revision the cache does not have: the task is read"
  );
  assert_eq!(
    workspace.task(task).expect("cached").revision,
    TaskRevision::try_from(3).expect("3")
  );
  assert_eq!(
    workspace.link().cursor,
    server.cursor(),
    "the cursor moved with the read"
  );

  server.revoke(ALICE);
  let stopped = tokio::time::timeout(SETTLED, stream).await.expect("stopped at once");
  assert_eq!(stopped.expect("the stream's task"), Stopped::NotMember);
  assert!(workspace.link().removed_at.is_some(), "the list reads not a member");
  assert_eq!(server.connects().len(), 1, "a revoked stream does not connect again");
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_the_desktops_state_is_live_reconnecting_not_a_member_and_none_once_local() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &["Status bar"]);
  let desktop = Desktop::signed_in(&server, ALICE, Fixture::new(), &[]);
  let streams = tokio::spawn(orchester_workspace::sharing::sync::run(
    desktop.servers.clone(),
    Runners::new(
      "no-runner".into(),
      tokio::runtime::Handle::current(),
      Activity::new(),
      SessionEnd::Clean,
    ),
  ));
  let state = || desktop.servers.sync_state(desktop.root());
  seen_within(SETTLED, "live", || state() == Some(SyncState::Live)).await;
  assert_eq!(desktop.servers.sync_states().len(), 1, "one state, the project's");

  server.refuse_streams(1);
  server.drop_streams();
  seen_within(SETTLED, "reconnecting", || {
    matches!(state(), Some(SyncState::Reconnecting { .. }))
  })
  .await;
  // About a second, then a refused connect and two more.
  seen_within(Duration::from_secs(5), "live again", || {
    state() == Some(SyncState::Live)
  })
  .await;
  reading_events(&server, || {
    let db = ProjectDb::open(desktop.root()).expect("the project database");
    db.cached_task(server.task_ids()[0]).expect("the cache")
  })
  .await;

  server.revoke(ALICE);
  seen_within(SETTLED, "not a member", || state() == Some(SyncState::NotMember)).await;

  ProjectDb::open(desktop.root())
    .expect("the project database")
    .make_local()
    .expect("made local");
  desktop.servers.retry_now(desktop.server);
  seen_within(SETTLED, "no state once local", || state().is_none()).await;
  streams.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_workflow_status_the_server_could_not_take_goes_with_the_minutes_round() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let mut fixture = Fixture::new();
  let task = fixture.task("Status bar", support::at(10));
  let mut run = fixture.started_run(task, support::at(11));
  let claimed = server.claimed(task, "Status bar", ALICE);
  let mut desktop = Desktop::signed_in(&server, ALICE, fixture, &[claimed]);
  desktop.fixture.succeed(&mut run, support::at(12));
  let waiting = || {
    let db = ProjectDb::open(desktop.root()).expect("the project database");
    db.automatic_statuses().expect("the waiting statuses")
  };
  assert_eq!(waiting().len(), 1, "the run moved its task to In review");

  // The run's change is sent at once, while the server cannot take it.
  server.refuse_requests(true);
  desktop.servers.send_automatic_statuses().await;
  assert_eq!(waiting().len(), 1, "it waits");
  assert_eq!(server.workflow(task), WorkflowStatus::Todo);

  // The server takes requests again; nobody asks to sync and no run changes.
  server.refuse_requests(false);
  let (_runs, changes) = tokio::sync::watch::channel(0);
  let work = tokio::spawn(orchester_workspace::sharing::project::work::run(
    desktop.servers.clone(),
    changes,
    Programs::processes(),
  ));
  let within = Duration::from_secs(5);
  seen_within(within, "the status on the server", || {
    server.workflow(task) == WorkflowStatus::InReview
  })
  .await;
  seen_within(within, "nothing waits", || waiting().is_empty()).await;
  work.abort();
}
