//! Removing what a run left as the desktop asks it ([`orchester_ipc::Command::RemoveWorktrees`], protocol 43) against
//! `orchester-fake-codex`: the runner keeps a worktree whose uncommitted changes nobody confirmed, removes it with
//! exactly the changes the person saw, and with everything the run left once a task is deleted; a run it cannot read
//! removes nothing. The desktop records the removal; the runner records nothing. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test worktree_removal`.

mod support;

use std::{fs, path::Path, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, RunId},
  integration::{
    Cleanup, KeepReason, Leftovers, leftovers,
    removal::{Discard, RemovalFailure, RemovalRequest},
  },
  store::{RunStore, WorktreeStore},
};
use orchester_exec::ProcessRunner;
use orchester_ipc::Reply;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;

use self::support::{Setup, project, scheduler, wait_for};

fn worktree(root: &Path, run: RunId) -> std::path::PathBuf {
  root.join(".orchester/worktrees").join(run.to_string())
}

fn left_by(root: &Path, id: RunId) -> Leftovers {
  let run = ProjectDb::open(root)
    .expect("open")
    .run(id)
    .expect("read")
    .expect("run");
  let git = ProcessRunner::new(Duration::from_secs(30));
  leftovers(&git, Path::new("git"), root, &run).expect("what the run left")
}

async fn remove(scheduler: &Scheduler, run: RunId, discard: Discard) -> Reply {
  let request = RemovalRequest {
    discard,
    retry_held: true,
  };
  scheduler.clone().remove_worktrees(run, request).await
}

fn removed(reply: Reply) -> Cleanup {
  match reply {
    Reply::WorktreesRemoved(cleanup) => cleanup,
    other => panic!("the runner removed: {other:?}"),
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn worktree_removal_through_the_runner_removes_only_the_changes_the_person_saw() {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
  let folder = worktree(root, run);
  fs::write(folder.join("notes.txt"), "not committed\n").expect("an uncommitted file");

  let kept = removed(remove(&scheduler, run, Discard::Seen(None)).await);
  assert!(folder.is_dir(), "changes nobody saw keep the worktree");
  assert!(
    matches!(kept.kept_worktrees.as_slice(), [(_, KeepReason::Uncommitted(files))] if files == &["notes.txt"]),
    "{kept:?}"
  );

  let seen = left_by(root, run);
  let cleanup = removed(remove(&scheduler, run, Discard::Seen(Some(seen))).await);
  assert!(cleanup.removed_run_worktree(), "{cleanup:?}");
  assert!(!folder.exists(), "the worktree with the changes the person saw went");
  assert!(
    git_branch_exists(root, run),
    "the run's branch, merged nowhere, stays: {cleanup:?}"
  );
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    db.worktree_removal(run).expect("read"),
    None,
    "the desktop records the removal, not the runner"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn worktree_removal_through_the_runner_removes_everything_for_a_deleted_task_and_refuses_an_unknown_run() {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
  fs::write(worktree(root, run).join("notes.txt"), "not committed\n").expect("an uncommitted file");

  let cleanup = removed(remove(&scheduler, run, Discard::Everything).await);
  assert!(!worktree(root, run).exists(), "{cleanup:?}");
  assert!(
    !git_branch_exists(root, run),
    "the unmerged branch went too: {cleanup:?}"
  );

  let unknown = remove(&scheduler, RunId::generate(), Discard::Everything).await;
  assert!(
    matches!(unknown, Reply::RemovalFailed(RemovalFailure::RunUnreadable(_))),
    "{unknown:?}"
  );
}

/// Whether the branch `run` worked on is still in the repository at `root`.
fn git_branch_exists(root: &Path, run: RunId) -> bool {
  let branch = ProjectDb::open(root)
    .expect("open")
    .run(run)
    .expect("read")
    .expect("run")
    .branch()
    .clone();
  let output = std::process::Command::new("git")
    .args(["branch", "--list", branch.as_str()])
    .current_dir(root)
    .output()
    .expect("git");
  !String::from_utf8_lossy(&output.stdout).trim().is_empty()
}
