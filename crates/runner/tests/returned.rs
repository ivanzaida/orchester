//! Returning the work of an ended run to a step (ORC-44) through the scheduler, with real worktrees and the fake CLI:
//! the run works again from that step with the note in its prompt; a merged run and one whose worktree is gone are
//! refused and stay as they were.
mod support;

use std::{fs, path::Path};

use orchester_core::{
  id::{IdempotencyKey, RunId},
  integration::{MergeKind, RunMerge},
  resource::StepKey,
  revision::Revision,
  run::{AttemptReason, BranchName, ReviewNote, Run, RunStatus},
  store::{MergeStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;
use support::{Setup, project, reported, scheduler, wait_for, wait_for_idle};

const NOTE: &str = "Merge main into this branch and resolve the conflicts in README.md.";

fn read(root: &Path, id: RunId) -> Run {
  ProjectDb::open(root)
    .expect("open")
    .run(id)
    .expect("read")
    .expect("run")
}

fn implement() -> StepKey {
  StepKey::new("implement").expect("a step key")
}

fn note() -> ReviewNote {
  ReviewNote::new(NOTE).expect("a note")
}

#[tokio::test(flavor = "multi_thread")]
async fn returned_a_succeeded_run_works_again_from_the_step_with_the_note() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let succeeded = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(succeeded.status(), &RunStatus::Succeeded);
  scheduler.return_ended(id, implement(), note()).await.expect("returned");
  let done = wait_for(root, &mut events, id, |run| {
    run.status() == &RunStatus::Succeeded && run.attempts().len() == 2
  })
  .await;
  wait_for_idle(&scheduler).await;
  assert_eq!(
    done.current_attempt().expect("attempt").reason(),
    &AttemptReason::ReturnedAfterEnd {
      return_to: implement(),
      note: note(),
    }
  );
  let db = ProjectDb::open(root).expect("open");
  let prompt = reported(&db, id, "prompt| ");
  assert!(prompt.iter().any(|line| line == NOTE), "{prompt:?}");
  assert!(
    prompt.iter().any(|line| line == "# Returned by the person"),
    "{prompt:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn returned_refuses_a_merged_run_and_one_whose_worktree_is_gone() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  let ended = read(root, id);

  let worktree = root.join(".orchester/worktrees").join(id.to_string());
  let kept = root.join(".orchester/kept-worktree");
  fs::rename(&worktree, &kept).expect("the worktree moved away");
  assert_eq!(
    scheduler.return_ended(id, implement(), note()).await,
    Err(Rejection::WorktreeRemoved)
  );
  fs::rename(&kept, &worktree).expect("the worktree back");

  ProjectDb::open(root)
    .expect("open")
    .save_run_merge(&RunMerge {
      run: id,
      into: BranchName::new("main").expect("branch"),
      revision: Revision::new(&"c".repeat(40)).expect("revision"),
      kind: MergeKind::FastForward,
      merged_at: Timestamp::now(),
    })
    .expect("merged");
  assert_eq!(
    scheduler.return_ended(id, implement(), note()).await,
    Err(Rejection::RunMerged)
  );
  assert_eq!(read(root, id), ended);
}
