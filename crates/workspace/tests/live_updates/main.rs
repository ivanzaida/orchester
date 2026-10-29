//! A shared project's live event stream against a fake team server on loopback (`docs/architecture/server.md`, "Live
//! updates", "Cursor and applying events"): two workspaces' caches see each other's task change, claim and run progress
//! within a second; a connect without a cursor applies the snapshot and a reconnect resumes from the cursor; a delta
//! the cache cannot take reads that task; `revoked` stops the stream and the list reads *not a member*; reconnects
//! wait 1 s doubling to 60 s with jitter, on a settable clock. The team's configuration comes with it ([`config`]).
//!
//! `cargo test -p orchester-workspace --test live_updates live_updates_`

mod config;
mod http;
mod server;
mod widening;

use std::{
  collections::HashMap,
  fs,
  path::PathBuf,
  sync::{Arc, Mutex},
  time::{Duration, Instant},
};

use orchester_api::{
  ClaimTake, RunReport, SharedTask, TaskChanges, TaskPatch, Versions,
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
  time::Timestamp,
};
use orchester_store::{CachedTask, ProjectDb, SharingLink};
use orchester_workspace::sharing::{
  SharingError,
  sync::{Clock, Source, Stopped, SystemClock, follow},
};
use server::TeamServer;
use tempfile::TempDir;

const ALICE: &str = "alice-token";
const BOB: &str = "bob-token";
/// How soon a change made on one workspace must reach the other's cache.
const LIVE: Duration = Duration::from_secs(1);

/// A member's view of the project's server: its client signed in with the member's token.
struct Member {
  client: Client,
  token: SecretValue,
  project: ProjectId,
}

impl Member {
  fn of(server: &TeamServer, token: &str) -> Self {
    let address = ServerAddress::parse(&server.url).expect("address");
    Self {
      client: Client::new(address, PlainHttp::NotAllowed).expect("client"),
      token: SecretValue::new(token.to_owned()).expect("token"),
      project: server.project,
    }
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

  async fn versions(&self, resource: ResourceId, after: Option<VersionNumber>) -> Result<Versions, SharingError> {
    Ok(self.client.versions(&self.token, self.project, resource, after).await?)
  }

  fn changed(&self) {}
}

/// A workspace's project linked to the server's project, its cache empty and without a cursor: the stream's snapshot
/// fills it.
struct Workspace {
  _repo: TempDir,
  root: PathBuf,
}

impl Workspace {
  fn linked(server: &TeamServer) -> Self {
    let repo = tempfile::tempdir().expect("temporary directory");
    fs::create_dir(repo.path().join(".git")).expect("create .git");
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("ORC").expect("key"),
      Name::new("Orchester").expect("name"),
    );
    let mut db = ProjectDb::create(repo.path(), project).expect("the project database");
    let link = SharingLink {
      server: ServerId::generate(),
      project: server.project,
      key: ProjectKey::new("ORC").expect("key"),
      role: Some(MemberRole::Member),
      joined_at: Timestamp::from_unix_millis(1),
      cursor: 0,
      last_synced_at: Timestamp::from_unix_millis(1),
      removed_at: None,
      address: Some("https://team.example/orchester".to_owned()),
    };
    db.start_sharing(&link, &[], (&[], &HashMap::new())).expect("linked");
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

/// Waits until `seen` holds, at most `within`; `what` names it when it does not.
async fn seen_within(within: Duration, what: &str, mut seen: impl FnMut() -> bool) {
  let start = Instant::now();
  while !seen() {
    assert!(start.elapsed() < within, "{what}: not seen within {within:?}");
    tokio::time::sleep(Duration::from_millis(10)).await;
  }
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
  seen_within(LIVE, "both snapshots", || {
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
  seen_within(LIVE, "the snapshot's cursor", || {
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
  seen_within(LIVE, "a change after the reconnect", || {
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
  seen_within(LIVE, "the snapshot", || workspace.task(task).is_some()).await;

  server.retitle_unannounced(task, "Missed");
  Member::of(&server, BOB).retitle(task, 2, "Status bar item").await;
  seen_within(LIVE, "the task read again", || {
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
  let stopped = tokio::time::timeout(LIVE, stream).await.expect("stopped at once");
  assert_eq!(stopped.expect("the stream's task"), Stopped::NotMember);
  assert!(workspace.link().removed_at.is_some(), "the list reads not a member");
  assert_eq!(server.connects().len(), 1, "a revoked stream does not connect again");
}

/// A clock whose waits pass at once and are kept, with a set jitter.
struct SettableClock {
  jitter: f64,
  waits: Mutex<Vec<Duration>>,
}

impl SettableClock {
  fn new(jitter: f64) -> Self {
    Self {
      jitter,
      waits: Mutex::new(Vec::new()),
    }
  }

  fn waits(&self) -> Vec<Duration> {
    self.waits.lock().expect("waits").clone()
  }
}

impl Clock for SettableClock {
  async fn wait(&self, wait: Duration) {
    self.waits.lock().expect("waits").push(wait);
    tokio::task::yield_now().await;
  }

  fn jitter(&self) -> f64 {
    self.jitter
  }
}

/// A server that cannot be reached `failures` times, then refuses the token.
struct Unreachable {
  failures: Mutex<usize>,
}

impl Source for Unreachable {
  async fn open(&self, _: Option<u64>) -> Result<EventStream, StreamEnd> {
    let mut failures = self.failures.lock().expect("failures");
    if *failures == 0 {
      return Err(StreamEnd::Unauthorized);
    }
    *failures -= 1;
    Err(StreamEnd::Failed(ClientError::Unreachable))
  }

  async fn task(&self, _: TaskId) -> Result<SharedTask, SharingError> {
    Err(SharingError::Offline)
  }

  async fn versions(&self, _: ResourceId, _: Option<VersionNumber>) -> Result<Versions, SharingError> {
    Err(SharingError::Offline)
  }

  fn changed(&self) {}
}

fn seconds(waits: &[f64]) -> Vec<Duration> {
  waits.iter().map(|wait| Duration::from_secs_f64(*wait)).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_reconnects_wait_one_second_doubling_to_sixty_with_jitter() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &[]);
  let workspace = Workspace::linked(&server);
  for (jitter, share) in [(0.0, 1.0), (1.0, 0.8), (0.5, 0.9)] {
    let clock = SettableClock::new(jitter);
    let source = Unreachable {
      failures: Mutex::new(8),
    };
    assert_eq!(follow(&source, &workspace.root, &clock).await, Stopped::SignedOut);
    let expected: Vec<f64> = [1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 60.0, 60.0]
      .iter()
      .map(|wait| wait * share)
      .collect();
    assert_eq!(clock.waits(), seconds(&expected), "jitter {jitter}");
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_a_stream_that_opened_starts_the_waits_over() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &["Status bar"]);
  let workspace = Workspace::linked(&server);
  server.refuse_streams(3);
  let clock = Arc::new(SettableClock::new(0.0));
  let following = {
    let (clock, root, member) = (clock.clone(), workspace.root.clone(), Member::of(&server, ALICE));
    tokio::spawn(async move { follow(&member, &root, &*clock).await })
  };
  seen_within(LIVE, "the snapshot after three refusals", || {
    workspace.link().cursor == server.cursor()
  })
  .await;
  assert_eq!(clock.waits(), seconds(&[1.0, 2.0, 4.0]));
  server.drop_streams();
  seen_within(LIVE, "the reconnect", || server.connects().len() == 5).await;
  assert_eq!(
    clock.waits(),
    seconds(&[1.0, 2.0, 4.0, 1.0]),
    "the dropped stream had opened"
  );
  server.revoke(ALICE);
  let stopped = tokio::time::timeout(LIVE, following).await.expect("stopped");
  assert_eq!(stopped.expect("the stream's task"), Stopped::NotMember);
}
