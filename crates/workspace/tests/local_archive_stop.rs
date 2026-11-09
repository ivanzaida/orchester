//! What archiving does to the runs that use what is archived (ORC-842): with no runner to stop them they are counted
//! as left running and nothing is recorded as stopped; with a runner that cancels them they are stopped and recorded
//! for the notification; and a run that starts while the archive is written is either one of the runs it reads or
//! refused by the store, so none keeps working on what is archived unseen.
//!
//! `cargo test -p orchester-workspace --test local_archive_stop archive_stop`

#[path = "support/runner_programs.rs"]
mod runner_programs;
mod support;

use std::{path::Path, sync::Arc, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  resource::ResourceKind,
  run::{BranchName, NewRun},
  store::{ResourceStore, RunStore},
};
use orchester_ipc::SessionEnd::Clean;
use orchester_store::{ProjectDb, RunStoreError};
use orchester_workspace::{
  activity::Activity,
  local_archive::{Archivable, archive},
  runner::{Runners, state::RunnerState},
  workspace::OpenProject,
};
use runner_programs::programs;
use support::{Fixture, at};
use tokio::{runtime::Handle, time::timeout};

const PATIENCE: Duration = Duration::from_secs(30);

fn builder(db: &ProjectDb) -> ResourceId {
  db.resources(ResourceKind::AgentProfile).expect("profiles")[0].id()
}

fn open(root: &Path, db: &ProjectDb) -> OpenProject {
  OpenProject {
    project: db.project().clone(),
    repo_root: root.to_owned(),
  }
}

/// Runners whose program does not exist: no runner ever attaches, so nothing can stop a run.
fn without_a_runner(root: &Path) -> Runners {
  Runners::new(
    root.join("no-such-runner.exe"),
    Handle::current(),
    Activity::new(),
    Clean,
  )
}

#[tokio::test(flavor = "multi_thread")]
async fn archive_stop_a_run_no_runner_can_stop_is_counted_as_left_running_and_not_recorded() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Search", at(10));
  fixture.started_run(task, at(11));
  let root = fixture.repo.path().to_owned();
  let profile = builder(&fixture.db);
  let runners = without_a_runner(&root);
  runners.show(Some(&open(&root, &fixture.db)));

  let archived = timeout(PATIENCE, archive(&runners, &root, Archivable::Resource(profile)))
    .await
    .expect("in time")
    .expect("archived");

  assert_eq!(archived.stopped, 0);
  assert_eq!(
    archived.left_running, 1,
    "the run keeps working on what is archived, and it is said"
  );
  assert!(archived.left_running_message().is_some());
  let db = ProjectDb::open(&root).expect("open");
  assert!(
    db.archive_stops().expect("stops").is_empty(),
    "no stop is recorded for a run that was not stopped"
  );
  assert!(
    db.resources(ResourceKind::AgentProfile).expect("profiles").is_empty(),
    "the archive itself was written"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn archive_stop_a_run_the_runner_cancels_is_stopped_and_recorded() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Search", at(10));
  let run = fixture.started_run(task, at(11));
  let root = fixture.repo.path().to_owned();
  let profile = builder(&fixture.db);
  let runners = Runners::new(programs().runner.clone(), Handle::current(), Activity::new(), Clean);
  let mut updates = runners.subscribe();
  runners.show(Some(&open(&root, &fixture.db)));
  timeout(PATIENCE, async {
    loop {
      if updates.borrow_and_update().state_for(&root) == RunnerState::Online {
        return;
      }
      updates.changed().await.expect("the runners outlive the test");
    }
  })
  .await
  .expect("the runner is online");

  let archived = timeout(PATIENCE, archive(&runners, &root, Archivable::Resource(profile)))
    .await
    .expect("in time")
    .expect("archived");

  assert_eq!((archived.stopped, archived.left_running), (1, 0), "{archived:?}");
  let stop = ProjectDb::open(&root)
    .expect("open")
    .archive_stop_of(run.id())
    .expect("read")
    .expect("the stop is recorded for the notification");
  assert_eq!(stop.task, task);
  assert_eq!(stop.resource.as_str(), "Builder");
  runners.shutdown(None).await;
}

/// Runs start while the archive is written. Each one either exists when the archive reads the runs that use the
/// profile, and is counted, or is refused by the store: the count equals the runs that exist afterwards.
#[tokio::test(flavor = "multi_thread")]
async fn archive_stop_a_run_started_while_archiving_is_counted_or_refused() {
  let mut fixture = Fixture::new();
  let task = fixture.task("Search", at(10));
  let root = fixture.repo.path().to_owned();
  let profile = builder(&fixture.db);
  let new = fixture.new_run_for(task);
  let runners = without_a_runner(&root);
  runners.show(Some(&open(&root, &fixture.db)));
  let starter = {
    let (root, new) = (root.clone(), Arc::new(new));
    std::thread::spawn(move || {
      let mut db = ProjectDb::open(&root).expect("open");
      let mut started = 0_usize;
      loop {
        let next = NewRun {
          branch: BranchName::new(&format!("orchester/orc-run-s{started}")).expect("branch"),
          ..(*new).clone()
        };
        match db.create_run(IdempotencyKey::generate(), next, at(20)) {
          Ok(_) => started += 1,
          Err(RunStoreError::UsesArchived(_)) => return started,
          Err(other) => panic!("a start is accepted or refused for what is archived, not {other:?}"),
        }
      }
    })
  };
  // Let a few runs start before the archive is written.
  std::thread::sleep(Duration::from_millis(100));

  let archived = timeout(PATIENCE, archive(&runners, &root, Archivable::Resource(profile)))
    .await
    .expect("in time")
    .expect("archived");
  let started = starter
    .join()
    .expect("the starter ended: the store refused a start after the archive");

  assert!(started > 0, "some runs started before the archive");
  let unfinished = ProjectDb::open(&root)
    .expect("open")
    .unfinished_runs()
    .expect("runs")
    .len();
  assert_eq!(
    unfinished, started,
    "every run that exists was started before the archive"
  );
  assert_eq!(
    archived.left_running, started,
    "every one of them was read by the archive, whichever side of it its start fell on"
  );
}
