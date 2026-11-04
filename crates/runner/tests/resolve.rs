//! Resolving by hand in a run's worktree as the runner carries it out for the desktop (`Command::Resolve`,
//! `docs/architecture/remote-runners.md` rule 5), with a real worktree and the fake CLI: the runner reads the worktree,
//! merges the base branch into the run's branch there and refuses what git refuses, with git's words.
mod support;

use std::fs;

use orchester_core::{
  id::{IdempotencyKey, RunId},
  integration::resolve::{
    Started,
    request::{FailureReason, ResolveAnswer, ResolveRequest},
  },
};
use support::{Setup, git, project, scheduler, wait_for, wait_for_idle};

#[tokio::test(flavor = "multi_thread")]
async fn resolve_merges_the_base_into_the_run_worktree_and_answers_what_it_found() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let ended = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  let base = ended.base_branch().expect("the run's base branch").clone();

  assert_eq!(
    scheduler.resolve(id, ResolveRequest::Read).await,
    Ok(ResolveAnswer::Read {
      present: true,
      merge: None
    })
  );
  assert_eq!(
    scheduler.resolve(id, ResolveRequest::InMerge).await,
    Ok(ResolveAnswer::InMerge(false))
  );
  let head = match scheduler.resolve(id, ResolveRequest::Head).await {
    Ok(ResolveAnswer::Head(head)) => head,
    other => panic!("the worktree's head: {other:?}"),
  };
  let worktree = root.join(".orchester/worktrees").join(id.to_string());
  assert_eq!(head.as_str(), git(&worktree, &["rev-parse", "HEAD"]));

  // The base branch moves on in a file the run did not touch: git merges it without a conflict.
  fs::write(root.join("from-main.txt"), "main moved on\n").expect("a file on main");
  git(root, &["add", "from-main.txt"]);
  git(root, &["commit", "-m", "main moves on"]);
  let start = ResolveRequest::Start {
    base,
    branch: ended.branch().clone(),
    expected: head,
  };
  let merged = match scheduler.resolve(id, start).await {
    Ok(ResolveAnswer::Started(Started::Merged(merged))) => merged,
    other => panic!("merged at once: {other:?}"),
  };
  assert_eq!(merged.base.as_str(), git(root, &["rev-parse", "HEAD"]));
  assert_eq!(merged.tip.as_str(), git(&worktree, &["rev-parse", "HEAD"]));

  let aborted = scheduler.resolve(id, ResolveRequest::Abort).await;
  assert_eq!(
    aborted.map_err(|failure| failure.reason),
    Err(FailureReason::NotInMerge),
    "no merge waits"
  );
  assert_eq!(
    scheduler.resolve(RunId::generate(), ResolveRequest::Present).await,
    Ok(ResolveAnswer::Present(false)),
    "a run without a worktree"
  );
}
