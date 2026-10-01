//! Recovery preserves history while invalidated dependent agents receive a fresh provider session.
mod support;

use std::{fs, num::NonZeroU32};

use orchester_core::{
  id::IdempotencyKey,
  resource::StepKey,
  run::RunStatus,
  store::{ActivityStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::AttachSession;
use orchester_store::ProjectDb;
use support::{Setup, StepSpec, project, scheduler, threads, wait_for, wait_for_idle};

#[tokio::test(flavor = "multi_thread")]
async fn recovery_sessions_new_upstream_work_starts_fresh_dependent_cli_and_keeps_history() {
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Append a line.", &[]),
      StepSpec::new("review", "Review", "Report failure.", &["implement"]),
    ],
    ..Setup::new("Say hello.")
  });
  let root = project.repo.path();
  let (old, mut events) = scheduler(root);
  let id = old
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let mut failed = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&old).await;
  assert!(matches!(failed.status(), RunStatus::Failed(_)));
  let mut db = ProjectDb::open(root).expect("open");
  let before = db.invocations(id).expect("original invocations");
  let path = root.join(".orchester/worktrees").join(id.to_string());
  fs::write(path.join("keep-uncommitted.txt"), "preserve review progress").expect("partial work");
  let recovery = failed
    .recover(&StepKey::new("implement").expect("step"), Timestamp::now())
    .expect("recover upstream");
  db.append_events(&failed, &recovery).expect("save recovery");
  drop(old);
  let (next, mut events) = scheduler(root);
  next
    .attach(AttachSession::Reconnect, NonZeroU32::MIN)
    .await
    .expect("attach");
  let ended = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&next).await;
  let db = ProjectDb::open(root).expect("reopen");
  let calls = db.invocations(id).expect("all invocations");
  assert_eq!(calls.len(), 4);
  assert_eq!(&calls[..2], before.as_slice(), "original sessions retained");
  let threads = threads(&db, id);
  assert_eq!(threads.len(), 4);
  assert!(
    threads[2].get("threadId").is_some(),
    "selected Implement resumes: {}",
    threads[2]
  );
  assert!(
    threads[3].get("threadId").is_none(),
    "dependent Review starts fresh: {}",
    threads[3]
  );
  assert_eq!(ended.id(), failed.id());
  assert_eq!(ended.branch(), failed.branch());
  assert_eq!(ended.attempts().len(), 2);
  assert_eq!(
    fs::read_to_string(path.join("keep-uncommitted.txt")).expect("saved work"),
    "preserve review progress"
  );
  assert!(
    matches!(ended.status(), RunStatus::Failed(_)),
    "fresh Review can still fail honestly"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn recovery_sessions_parallel_review_uses_current_source_and_preserves_its_earlier_worktree() {
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Write own file.", &[]),
      StepSpec::new("checks", "Checks", "Write own file.", &["implement"]),
      StepSpec::new(
        "review",
        "Review",
        "Write own file. Report failure. Work slowly.",
        &["implement"],
      ),
    ],
    ..Setup::new("Say hello.")
  });
  let root = project.repo.path();
  let (old, mut events) = scheduler(root);
  let id = old
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let mut failed = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&old).await;
  let review_key = StepKey::new("review").expect("key");
  let old_review = failed.latest_execution(&review_key).expect("old review").id();
  let old_path = root.join(".orchester/worktrees").join(old_review.to_string());
  assert!(old_path.is_dir());
  fs::write(old_path.join("keep-review.txt"), "saved older review work").expect("partial work");
  let old_source = fs::read_to_string(old_path.join("README.md")).expect("old source");
  assert!(!old_source.contains("continued"));
  let mut db = ProjectDb::open(root).expect("open");
  let before = db.invocations(id).expect("original invocations");
  let recovery = failed
    .recover(&StepKey::new("implement").expect("step"), Timestamp::now())
    .expect("recover");
  db.append_events(&failed, &recovery).expect("save recovery");
  drop(old);
  let (next, mut events) = scheduler(root);
  next
    .attach(AttachSession::Reconnect, NonZeroU32::new(2).expect("slots"))
    .await
    .expect("attach");
  let ended = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&next).await;
  assert!(matches!(ended.status(), RunStatus::Failed(_)));
  let new_review = ended.latest_execution(&review_key).expect("new review").id();
  let new_path = root.join(".orchester/worktrees").join(new_review.to_string());
  assert_ne!(new_path, old_path);
  assert!(
    fs::read_to_string(new_path.join("README.md"))
      .expect("current source")
      .contains("continued")
  );
  assert!(!new_path.join("keep-review.txt").exists());
  assert_eq!(
    fs::read_to_string(old_path.join("README.md")).expect("old source retained"),
    old_source
  );
  assert_eq!(
    fs::read_to_string(old_path.join("keep-review.txt")).expect("old work retained"),
    "saved older review work"
  );
  let db = ProjectDb::open(root).expect("reopen");
  let calls = db.invocations(id).expect("all invocations");
  assert_eq!(calls.len(), 6);
  assert_eq!(&calls[..3], before.as_slice());
  assert_eq!(
    threads(&db, id)
      .iter()
      .filter(|thread| thread.get("threadId").is_some())
      .count(),
    1
  );
}
