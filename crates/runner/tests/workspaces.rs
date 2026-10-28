//! Warm workspaces end to end (ORC-114): a run borrows one of the project's workspaces, gives it back detached and
//! clean once it settles, the next run takes the one idle longest, a run waits while every workspace is in use, a
//! workspace with another run's leftovers is stranded instead of switched, and below the free space the project keeps
//! no run starts, agents at work stop with their work committed and the run pauses until there is room — unless the
//! project has not turned workspaces on. Against `orchester-fake-codex`; needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test workspaces`.

mod support;

use std::{fs, num::NonZeroU32, sync::Arc, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, RunId},
  limits::{AgentSlots, ProjectLimits},
  run::{PauseReason, RunStatus},
  store::{ActivityStore, SettingsStore, WorkspaceStore},
  time::Timestamp,
  workspaces::{Holder, Stranded, Warmth, WorkspaceNumber},
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use self::support::{
  Setup, StepSpec,
  disk::{TestDisk, pooled_scheduler},
  git, project, queue_another_task, scheduler_on,
  vault::MemoryVault,
  wait_for, wait_for_idle, wait_for_turns,
};

/// How often the disk guard looks in the tests that watch it work, instead of every 30 seconds.
const QUICK: Duration = Duration::from_millis(100);

/// Free gigabytes below the 150 the project keeps free by default.
const LOW_GB: u64 = 92;

fn number(n: u32) -> WorkspaceNumber {
  WorkspaceNumber::new(n).expect("above zero")
}

fn read(root: &std::path::Path, id: RunId) -> orchester_core::run::Run {
  use orchester_core::store::RunStore;
  ProjectDb::open(root)
    .expect("open")
    .run(id)
    .expect("read")
    .expect("the run")
}

fn agents_at_once(root: &std::path::Path, count: u64) {
  let mut db = ProjectDb::open(root).expect("open");
  let limits = ProjectLimits::default().with_agent_slots(AgentSlots::new(count).expect("in range"));
  db.save_project_limits(&limits).expect("save limits");
}

#[tokio::test(flavor = "multi_thread")]
async fn workspaces_a_run_borrows_one_and_gives_it_back_detached_with_its_work_on_its_branch() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let (scheduler, mut events) = pooled_scheduler(root, TestDisk::default());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  assert!(!root.join(".orchester/worktrees").join(id.to_string()).exists());
  let records = ProjectDb::open(root).expect("open").workspaces().expect("workspaces");
  assert_eq!(records.len(), 1);
  assert_eq!(
    (
      records[0].number,
      records[0].holder,
      records[0].last_run,
      records[0].warmth
    ),
    (number(1), None, Some(id), Warmth::Warm)
  );
  let workspace = number(1).path(root);
  assert_eq!(git(&workspace, &["status", "--porcelain"]), "", "given back clean");
  assert_eq!(
    git(&workspace, &["rev-parse", "--abbrev-ref", "HEAD"]),
    "HEAD",
    "detached"
  );
  assert_eq!(
    git(root, &["rev-parse", done.branch().as_str()]),
    git(&workspace, &["rev-parse", "HEAD"])
  );

  let second = queue_another_task(root);
  let next = scheduler
    .start_run(IdempotencyKey::generate(), second)
    .await
    .expect("start the next run");
  wait_for(root, &mut events, next, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  let records = ProjectDb::open(root).expect("open").workspaces().expect("workspaces");
  assert_eq!(records.len(), 1, "the warm workspace is reused");
  assert_eq!(records[0].last_run, Some(next));
}

#[tokio::test(flavor = "multi_thread")]
async fn workspaces_a_run_waits_while_every_workspace_is_in_use_and_goes_on_once_one_is_free() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  agents_at_once(root, 1);
  let limit = NonZeroU32::MIN;
  ProjectDb::open(root)
    .expect("open")
    .borrow_workspace(Holder::WarmUp, None, limit, Timestamp::now())
    .expect("the warm-up holds the only workspace");
  let (scheduler, mut events) = pooled_scheduler(root, TestDisk::default());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  tokio::time::timeout(support::PATIENCE, async {
    while ProjectDb::open(root)
      .expect("open")
      .workspace_waits()
      .expect("waits")
      .is_empty()
    {
      tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
  })
  .await
  .expect("the run waits for a workspace");
  assert_eq!(read(root, id).status(), &RunStatus::Running);
  ProjectDb::open(root)
    .expect("open")
    .return_workspace(Holder::WarmUp, Timestamp::now())
    .expect("the warm-up gives it back");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  assert!(
    ProjectDb::open(root)
      .expect("open")
      .workspace_waits()
      .expect("waits")
      .is_empty()
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn workspaces_leftovers_of_another_run_strand_a_workspace_and_the_run_takes_the_next() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  agents_at_once(root, 2);
  let (scheduler, mut events) = pooled_scheduler(root, TestDisk::default());
  let first = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  wait_for(root, &mut events, first, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  fs::write(number(1).path(root).join("left-behind.txt"), "a crashed run's work").expect("a leftover");
  let other = RunId::generate();
  let mut db = ProjectDb::open(root).expect("open");
  db.borrow_workspace(Holder::Run { run: other }, None, NonZeroU32::MIN, Timestamp::now())
    .expect("another run used it");
  db.return_workspace(Holder::Run { run: other }, Timestamp::now())
    .expect("and gave it back");
  let second = queue_another_task(root);
  let next = scheduler
    .start_run(IdempotencyKey::generate(), second)
    .await
    .expect("start the next run");
  let done = wait_for(root, &mut events, next, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  let records = ProjectDb::open(root).expect("open").workspaces().expect("workspaces");
  assert_eq!(
    records[0].stranded,
    Some(Stranded {
      run: Some(other),
      files: 1
    })
  );
  assert!(
    number(1).path(root).join("left-behind.txt").is_file(),
    "nothing is thrown away unseen"
  );
  assert_eq!(records[1].last_run, Some(next));
}

#[tokio::test(flavor = "multi_thread")]
async fn workspaces_disk_guard_refuses_new_work_below_what_the_project_keeps_free() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let disk = TestDisk::default();
  disk.set_free(LOW_GB);
  let (scheduler, _events) = pooled_scheduler(root, disk.clone());
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), project.task).await,
    Err(Rejection::LowDisk {
      free_gb: LOW_GB,
      needed_gb: 150
    })
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn workspaces_disk_guard_pauses_a_working_step_with_a_checkpoint_and_resumes_once_there_is_room() {
  let project = project(&Setup::new("Wait for release. Draft first. Then keep working."));
  let root = project.repo.path();
  let disk = TestDisk::default();
  let (scheduler, mut events) = pooled_scheduler(root, disk.clone());
  let scheduler = scheduler.with_disk_checks(QUICK);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  wait_for_turns(root, &mut events, id, 1).await;
  let readme = number(1).path(root).join("README.md");
  tokio::time::timeout(support::PATIENCE, async {
    while !fs::read_to_string(&readme).is_ok_and(|text| text.contains("draft")) {
      tokio::time::sleep(Duration::from_millis(50)).await;
    }
  })
  .await
  .expect("the agent drafts in its workspace");
  disk.set_free(LOW_GB);
  let paused = wait_for(root, &mut events, id, |run| run.status() != &RunStatus::Running).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(paused.status(), &RunStatus::Paused(PauseReason::LowDisk));
  assert!(
    git(root, &["show", &format!("{}:README.md", paused.branch().as_str())]).contains("draft"),
    "the draft was committed before the workspace was given back"
  );
  let records = ProjectDb::open(root).expect("open").workspaces().expect("workspaces");
  assert!(records.iter().all(|record| record.holder.is_none()), "{records:?}");

  // The watch over paused runs resumes it by itself once there is room.
  disk.set_free(10_000);
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  let records = ProjectDb::open(root).expect("open").workspaces().expect("workspaces");
  assert_eq!(
    records[0].last_run,
    Some(id),
    "it went on in the workspace it used last"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn workspaces_disk_guard_stops_a_step_beside_one_that_ends_while_the_disk_is_low() {
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Work slowly. Write own file.", &[]),
      StepSpec::new("peer", "Peer", "Keep working.", &[]),
    ],
    ..Setup::new("Complete independent steps.")
  });
  let root = project.repo.path();
  let disk = TestDisk::default();
  // The guard looks only every 30 seconds while agents work: the step that ends finds the disk low first.
  let (scheduler, mut events) = pooled_scheduler(root, disk.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  wait_for_turns(root, &mut events, id, 2).await;
  disk.set_free(LOW_GB);
  let paused = tokio::time::timeout(
    Duration::from_secs(15),
    wait_for(root, &mut events, id, |run| run.status() != &RunStatus::Running),
  )
  .await
  .expect("the peer is stopped once the other step ends, not at the next look");
  wait_for_idle(&scheduler).await;
  assert_eq!(paused.status(), &RunStatus::Paused(PauseReason::LowDisk));
  let records = ProjectDb::open(root).expect("open").workspaces().expect("workspaces");
  assert!(records.iter().all(|record| record.holder.is_none()), "{records:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn workspaces_disk_guard_holds_nothing_back_while_the_project_has_not_turned_workspaces_on() {
  let project = project(&Setup::new("Work slowly."));
  let root = project.repo.path();
  let disk = TestDisk::default();
  disk.set_free(LOW_GB);
  let (scheduler, mut events) = scheduler_on(root, Arc::new(MemoryVault::default()), disk);
  let scheduler = scheduler.with_disk_checks(QUICK);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the guard does not refuse it");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(db.invocations(id).expect("invocations").len(), 1, "never paused");
  assert!(db.workspaces().expect("workspaces").is_empty(), "no workspace borrowed");
  assert!(
    root.join(".orchester/worktrees").join(id.to_string()).is_dir(),
    "its own worktree"
  );
}
