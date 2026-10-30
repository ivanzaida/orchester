//! The disk guard in every project (ORC-422): below the free space the project keeps — 50 GB until a person sets it —
//! no run starts and working steps pause with their work committed, whether the project's runs borrow warm workspaces
//! or own a worktree; a changed keep-free applies at the guard's next check; a step that failed while the disk was
//! low is paused, not failed; paused runs go on by themselves once there is room; and what could be cleaned is listed
//! in the order to clean. Against `orchester-fake-codex`; needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test disk_guard`.

mod support;

use std::{fs, path::PathBuf, sync::Arc, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, RunId},
  run::{PauseReason, RunStatus},
  store::{SettingsStore, WorkspaceStore},
  workspaces::{Cleanable, CleanableKind, GIGABYTE, KeepFree, WorkspaceNumber, cleanable_order},
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use self::support::{
  Setup,
  disk::{TestDisk, run_folder},
  git, project, scheduler_on,
  vault::MemoryVault,
  wait_for, wait_for_idle, wait_for_turns,
};

/// How often the disk guard looks in the tests that watch it work, instead of every 30 seconds.
const QUICK: Duration = Duration::from_millis(100);

fn keep_free(root: &std::path::Path, gigabytes: u64) {
  ProjectDb::open(root)
    .expect("open")
    .save_keep_free(KeepFree::from_gigabytes(gigabytes).expect("in range"))
    .expect("save keep free");
}

/// Waits until the agent of run `id` has drafted in its folder.
async fn drafted(root: &std::path::Path, id: RunId) {
  let readme = run_folder(root, id).join("README.md");
  tokio::time::timeout(support::PATIENCE, async {
    while !fs::read_to_string(&readme).is_ok_and(|text| text.contains("draft")) {
      tokio::time::sleep(Duration::from_millis(50)).await;
    }
  })
  .await
  .expect("the agent drafts in its folder");
}

#[tokio::test(flavor = "multi_thread")]
async fn disk_guard_pauses_a_run_without_warm_workspaces_and_resumes_it_once_there_is_room() {
  let project = project(&Setup::new("Wait for release. Draft first. Then keep working."));
  let root = project.repo.path();
  let disk = TestDisk::default();
  let (scheduler, mut events) = scheduler_on(root, Arc::new(MemoryVault::default()), disk.clone());
  let scheduler = scheduler.with_disk_checks(QUICK);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  wait_for_turns(root, &mut events, id, 1).await;
  drafted(root, id).await;
  assert!(
    root.join(".orchester/worktrees").join(id.to_string()).is_dir(),
    "its own worktree"
  );
  disk.set_free(32);
  let paused = wait_for(root, &mut events, id, |run| run.status() != &RunStatus::Running).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(paused.status(), &RunStatus::Paused(PauseReason::LowDisk));
  assert!(
    git(root, &["show", &format!("{}:README.md", paused.branch().as_str())]).contains("draft"),
    "the draft was committed when the run paused"
  );
  let db = ProjectDb::open(root).expect("open");
  assert!(db.workspaces().expect("workspaces").is_empty(), "no workspace borrowed");

  disk.set_free(10_000);
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(done.status(), &RunStatus::Succeeded, "it went on by itself");
}

#[tokio::test(flavor = "multi_thread")]
async fn disk_guard_keeps_50_gb_free_for_a_project_that_never_set_it() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let disk = TestDisk::default();
  disk.set_free(49);
  let (scheduler, mut events) = scheduler_on(root, Arc::new(MemoryVault::default()), disk.clone());
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), project.task).await,
    Err(Rejection::LowDisk {
      free_gb: 49,
      needed_gb: 50
    }),
    "the run waits with the reason"
  );
  disk.set_free(50);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("50 GB free is enough");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn disk_guard_applies_a_changed_keep_free_at_its_next_check() {
  let project = project(&Setup::new("Wait for release. Draft first. Then keep working."));
  let root = project.repo.path();
  let disk = TestDisk::default();
  disk.set_free(100);
  let (scheduler, mut events) = scheduler_on(root, Arc::new(MemoryVault::default()), disk);
  let scheduler = scheduler.with_disk_checks(QUICK);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  wait_for_turns(root, &mut events, id, 1).await;
  drafted(root, id).await;

  keep_free(root, 200);
  let paused = wait_for(root, &mut events, id, |run| run.status() != &RunStatus::Running).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(paused.status(), &RunStatus::Paused(PauseReason::LowDisk));

  keep_free(root, 60);
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(done.status(), &RunStatus::Succeeded, "it went on without a restart");
}

#[tokio::test(flavor = "multi_thread")]
async fn disk_guard_pauses_a_step_that_failed_while_the_disk_was_low_instead_of_failing_it() {
  let project = project(&Setup::new("Wait for release. Report failure."));
  let root = project.repo.path();
  let disk = TestDisk::default();
  // The guard looks only every 30 seconds while agents work: the step ends before it looks again.
  let (scheduler, mut events) = scheduler_on(root, Arc::new(MemoryVault::default()), disk.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  wait_for_turns(root, &mut events, id, 1).await;
  disk.set_free(32);
  fs::write(root.join(".orchester").join("fake-release"), "go").expect("release the agent");
  let paused = tokio::time::timeout(
    Duration::from_secs(15),
    wait_for(root, &mut events, id, |run| run.status() != &RunStatus::Running),
  )
  .await
  .expect("the step ends before the guard looks again");
  wait_for_idle(&scheduler).await;
  assert_eq!(paused.status(), &RunStatus::Paused(PauseReason::LowDisk));
  let execution = paused
    .current_attempt()
    .and_then(|attempt| attempt.executions().first())
    .expect("the step's execution");
  assert!(
    execution.status().is_open(),
    "it goes on once there is room, as the other tests watch: {:?}",
    execution.status()
  );
}

#[test]
fn disk_guard_lists_ended_runs_worktrees_first_then_idle_workspaces_each_largest_first() {
  let worktree = |bytes: Option<u64>| Cleanable {
    kind: CleanableKind::EndedRunWorktree { run: RunId::generate() },
    path: PathBuf::from("worktree"),
    bytes,
  };
  let workspace = |number: u32, bytes: Option<u64>| Cleanable {
    kind: CleanableKind::IdleWorkspace {
      number: WorkspaceNumber::new(number).expect("above zero"),
    },
    path: PathBuf::from("workspace"),
    bytes,
  };
  let small = worktree(Some(2 * GIGABYTE));
  let large = worktree(Some(30 * GIGABYTE));
  let unread = worktree(None);
  let huge_workspace = workspace(1, Some(90 * GIGABYTE));
  let small_workspace = workspace(2, Some(GIGABYTE));
  let ordered = cleanable_order(vec![
    small_workspace.clone(),
    small.clone(),
    unread.clone(),
    huge_workspace.clone(),
    large.clone(),
  ]);
  assert_eq!(ordered, [large, small, unread, huge_workspace, small_workspace]);
}
