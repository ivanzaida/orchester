//! The stream's reconnects (`docs/architecture/server.md`, "Live updates"): waits of 1 s doubling to 60 s with jitter,
//! on a settable clock, started over once a stream opened, and the project's state through *live*, *reconnecting*,
//! *offline* and *not a member*.

use super::*;

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
  configs: TeamConfigs,
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

  async fn resources(&self) -> Result<Vec<SharedResource>, SharingError> {
    Err(SharingError::Offline)
  }

  async fn versions(&self, _: ResourceId, _: Option<VersionNumber>) -> Result<Versions, SharingError> {
    Err(SharingError::Offline)
  }

  async fn report_catalog(&self, _: CatalogReport) -> Result<(), SharingError> {
    Err(SharingError::Offline)
  }

  async fn archived(&self, _: ResourceId, _: Option<&str>) {}

  fn changed(&self) {}

  fn state(&self, _: SyncState) {}

  fn configs(&self) -> &TeamConfigs {
    &self.configs
  }

  fn unreached(&self, _: &SharingError) -> Message {
    Message::new("sharing.errors.offline")
  }

  async fn retried(&self) {
    std::future::pending::<()>().await;
  }
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
      configs: TeamConfigs::default(),
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

#[tokio::test(flavor = "multi_thread")]
async fn live_updates_the_state_moves_through_live_reconnecting_offline_and_not_a_member() {
  let server = TeamServer::start(&[(ALICE, "Alice Moreau")], &["Status bar"]);
  let workspace = Workspace::linked(&server);
  let clock = Arc::new(SettableClock::new(0.0));
  let member = Arc::new(Member::of(&server, ALICE));
  let started = Timestamp::now();
  let following = {
    let (clock, root, member) = (clock.clone(), workspace.root.clone(), member.clone());
    tokio::spawn(async move { follow(&*member, &root, &*clock).await })
  };
  seen_within(LIVE, "the snapshot", || workspace.link().cursor == server.cursor()).await;
  assert_eq!(member.states(), [SyncState::Live], "live once the stream connects");

  // The stream drops, then six connects are refused before the seventh opens: the wait reaches a minute at the
  // seventh failure in a row, the drop counting as the first.
  server.refuse_streams(6);
  server.drop_streams();
  seen_within(LIVE, "live again after six refusals", || {
    server.connects().len() == 8 && member.states().last() == Some(&SyncState::Live)
  })
  .await;
  let states = member.states();
  let reconnecting: Vec<_> = (1..=6).map(|attempt| SyncState::Reconnecting { attempt }).collect();
  assert_eq!(states[..7], [&[SyncState::Live][..], &reconnecting].concat());
  let SyncState::Offline { since } = states[7] else {
    panic!("offline at the seventh failure in a row: {states:?}");
  };
  assert!(
    since >= started && since <= Timestamp::now(),
    "offline since the stream dropped"
  );
  assert_eq!(states[8..], [SyncState::Live], "live again once it connects");

  server.revoke(ALICE);
  let stopped = tokio::time::timeout(LIVE, following).await.expect("stopped");
  assert_eq!(stopped.expect("the stream's task"), Stopped::NotMember);
  assert_eq!(member.states().last(), Some(&SyncState::NotMember), "revoked");
}
