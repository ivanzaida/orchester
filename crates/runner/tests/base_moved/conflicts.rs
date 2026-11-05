//! A writing agent that ends its step with conflict markers left of the moved base the runner merged for it: the
//! step fails, says where the markers are, and nothing of them is committed; the merge stays in progress in the
//! worktree for a person or the next attempt.

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  run::RunStatus,
  store::ActivityStore,
};
use orchester_store::ProjectDb;

use super::{
  delivery, ended, implement,
  overlap::reworded_readme,
  release,
  support::{git, project, scheduler, wait_for, wait_for_turns},
  verify,
};

#[tokio::test(flavor = "multi_thread")]
async fn base_moved_conflict_markers_left_fail_the_step_and_are_never_committed() {
  let setup = delivery(
    implement("Wait for release."),
    verify("Compare the screen. Change nothing."),
  );
  let project = project(&setup);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(orchester_core::id::IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for_turns(root, &mut events, id, 1).await;
  let head = reworded_readme(root, (id, project.task));
  std::fs::write(root.join(".orchester").join("fake-leave-conflicts"), "left").expect("written");
  release(root);
  let run = wait_for(root, &mut events, id, |run| {
    ended(run) || matches!(run.status(), RunStatus::WaitingForPerson(_))
  })
  .await;
  assert!(!matches!(run.status(), RunStatus::Succeeded), "{:?}", run.status());

  let detail = ProjectDb::open(root)
    .expect("open")
    .activity(id, 0)
    .expect("activity")
    .into_iter()
    .find_map(|entry| match entry.event {
      AgentEvent::RunnerProblem {
        problem: RunnerProblem::CheckpointFailed { detail },
      } => Some(detail),
      _ => None,
    })
    .expect("the markers left are reported");
  assert!(detail.contains("conflict markers are left in README.md"), "{detail}");
  let branch = run.branch().as_str();
  let committed = git(root, &["show", &format!("{branch}:README.md")]);
  assert!(!committed.contains("<<<<<<<"), "{committed}");
  let merges = git(root, &["rev-list", "--merges", "--parents", branch]);
  assert!(!merges.contains(&head), "the merge was not committed: {merges}");
  let worktree = root.join(".orchester").join("worktrees").join(id.to_string());
  assert_eq!(git(&worktree, &["rev-parse", "MERGE_HEAD"]), head, "still in progress");
  let left = std::fs::read_to_string(worktree.join("README.md")).expect("read");
  assert!(left.contains("<<<<<<<"), "{left}");
}
