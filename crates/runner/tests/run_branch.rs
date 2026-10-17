//! The branch a run starts on, and a run whose worktree cannot be created. A branch of the run's own name may exist
//! already (an earlier task with the same key, a removed worktree's, one a person made): the run takes the first
//! free numbered name, records it, and leaves that branch as it was. Any other failure to create the worktree fails
//! the run with a reason in words of its own instead of leaving it pending; such a run owns no branch. Needs `git`
//! and the fake Codex CLI. Run with `cargo test -p orchester-runner --features test-agent --test run_branch`.

mod support;

use std::{fs, path::Path, sync::Barrier, thread, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, RunId},
  integration::{clean_up, leftovers},
  run::{Run, RunFailure, RunStatus, WorktreeFailure},
  store::RunStore,
};
use orchester_exec::ProcessRunner;
use orchester_ipc::{Event, Rejection};
use orchester_runner::worktree::Worktrees;
use orchester_store::ProjectDb;

use self::support::{Setup, git, pending::record_pending_run, project, scheduler, wait_for, wait_for_idle};

/// The branch the task's first run takes when its name is free.
const FIRST_RUN_BRANCH: &str = "orchester/orc-1-run-1";

fn stored(root: &Path, id: RunId) -> Run {
  ProjectDb::open(root)
    .expect("open")
    .run(id)
    .expect("read run")
    .expect("run exists")
}

/// A branch named `branch` with a commit of someone's own on it, off `main`; its commit.
fn someones_branch(root: &Path, branch: &str) -> String {
  let commit = git(
    root,
    &["commit-tree", "HEAD^{tree}", "-p", "HEAD", "-m", "someone's work"],
  );
  git(root, &["branch", branch, &commit]);
  commit
}

/// A `post-checkout` hook that prints a URL with credentials and fails, as a hook that misses a tool may. Git for
/// Windows runs it with its own `sh`, as Unix does.
fn failing_post_checkout_hook(root: &Path) {
  let hook = root.join(".git/hooks/post-checkout");
  fs::create_dir_all(hook.parent().expect("hooks folder")).expect("hooks folder");
  fs::write(
    &hook,
    "#!/bin/sh\necho 'fatal: cannot fetch https://someone:hunter2@example.com/lfs' >&2\nexit 1\n",
  )
  .expect("write hook");
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("make the hook executable");
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_taken_branch_name_starts_the_run_on_the_next_free_one_and_keeps_the_branch() {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  let theirs = someones_branch(root, FIRST_RUN_BRANCH);
  let removed_worktrees = someones_branch(root, &format!("{FIRST_RUN_BRANCH}-2"));
  let (scheduler, mut events) = scheduler(root);

  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts on a free branch");

  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  let chosen = format!("{FIRST_RUN_BRANCH}-3");
  assert_eq!(run.branch().as_str(), chosen, "the run records the branch it works on");
  let worktree = Worktrees::new(root).expect("worktrees").path(id);
  assert_eq!(git(&worktree, &["branch", "--show-current"]), chosen);
  assert_eq!(
    git(root, &["rev-parse", FIRST_RUN_BRANCH]),
    theirs,
    "the taken branch is untouched"
  );
  assert_eq!(
    git(root, &["rev-parse", &format!("{FIRST_RUN_BRANCH}-2")]),
    removed_worktrees,
    "every taken branch is untouched"
  );
  let db = ProjectDb::open(root).expect("open");
  let branches = db.run_branches().expect("run branches");
  assert_eq!(branches.len(), 1);
  assert_eq!(
    branches[0].branch.as_str(),
    chosen,
    "what pull requests and merging read"
  );
  let events = db.events(id).expect("events");
  assert_eq!(events[1].kind().name(), "branch_chosen", "chosen before it started");
  assert_eq!(Run::replay(&events).expect("replay").branch().as_str(), chosen);
  wait_for_idle(&scheduler).await;
}

#[test]
fn runs_racing_for_one_branch_name_each_take_a_branch_of_their_own() {
  const RUNS: usize = 4;
  let project = project(&Setup::new("Report failure."));
  let worktrees = Worktrees::new(project.repo.path()).expect("worktrees");
  let start = Barrier::new(RUNS);

  let mut branches: Vec<String> = thread::scope(|scope| {
    let racing: Vec<_> = (0..RUNS)
      .map(|_| {
        scope.spawn(|| {
          start.wait();
          worktrees
            .create(RunId::generate(), FIRST_RUN_BRANCH, "main")
            .expect("each run gets a worktree")
            .branch()
            .to_owned()
        })
      })
      .collect();
    racing.into_iter().map(|run| run.join().expect("no panic")).collect()
  });

  branches.sort();
  let mut expected: Vec<String> = std::iter::once(FIRST_RUN_BRANCH.to_owned())
    .chain((2..=RUNS).map(|suffix| format!("{FIRST_RUN_BRANCH}-{suffix}")))
    .collect();
  expected.sort();
  assert_eq!(branches, expected, "one name each, none shared");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_worktree_that_cannot_be_created_fails_the_run_with_the_reason_and_it_runs_again() {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  // Git would show a worktree in the user's checkout: the runner refuses to create it there.
  fs::write(root.join(".git/info/exclude"), "").expect("stop excluding .orchester");
  let (scheduler, mut events) = scheduler(root);
  let key = IdempotencyKey::generate();

  let refused = scheduler.start_run(key, project.task).await;

  let Err(Rejection::RunNotStarted { run: id, why }) = refused else {
    panic!("the start is refused with the reason, got {refused:?}");
  };
  assert_eq!(why, WorktreeFailure::NotIgnored, "the reason in words of its own");
  let run = stored(root, id);
  assert_eq!(
    run.status(),
    &RunStatus::Failed(RunFailure::WorktreeNotCreated(why.clone())),
    "failed, not pending"
  );
  assert!(run.attempts().is_empty(), "no step ran");
  assert!(!Worktrees::new(root).expect("worktrees").path(id).exists());
  assert!(scheduler.active_runs().is_empty(), "nothing waits for it");
  let told = std::iter::from_fn(|| events.try_recv().ok())
    .any(|event| matches!(event, Event::RunChanged { run, task } if run == id && task == project.task));
  assert!(told, "the screens are told the run changed");
  assert_eq!(
    scheduler.start_run(key, project.task).await,
    Err(Rejection::RunNotStarted { run: id, why }),
    "a retried request answers the same"
  );

  fs::write(root.join(".git/info/exclude"), ".orchester/\n").expect("exclude .orchester again");
  let again = scheduler
    .run_again(IdempotencyKey::generate(), project.task, id, None)
    .await
    .expect("the person starts it again");
  let next = wait_for(root, &mut events, again, |run| run.status().is_terminal()).await;
  assert_eq!(next.branch().as_str(), "orchester/orc-1-run-2");
  assert!(!next.attempts().is_empty(), "this one started");
  wait_for_idle(&scheduler).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pending_run_whose_worktree_exists_starts_on_the_branch_it_has() {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  let pending = record_pending_run(root, project.task).await;
  someones_branch(root, FIRST_RUN_BRANCH);
  // The start that recorded the run made its worktree on a free name, then stopped before recording it.
  let made = Worktrees::new(root)
    .expect("worktrees")
    .create(pending.id(), pending.branch().as_str(), pending.base().as_str())
    .expect("worktree");
  let chosen = format!("{FIRST_RUN_BRANCH}-2");
  assert_eq!(made.branch(), chosen);
  let (scheduler, mut events) = scheduler(root);

  let id = scheduler
    .start_run(project.task.first_run_key(), project.task)
    .await
    .expect("the retried start goes on in that worktree");

  assert_eq!(id, pending.id());
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.branch().as_str(), chosen, "the branch its worktree has");
  assert!(!run.attempts().is_empty(), "it started");
  wait_for_idle(&scheduler).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_that_fails_after_skipping_a_taken_name_owns_no_branch() {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  // Merged into `main`: cleanup would delete it, were it the run's.
  git(root, &["branch", FIRST_RUN_BRANCH]);
  failing_post_checkout_hook(root);
  let (scheduler, _events) = scheduler(root);

  let refused = scheduler.start_run(IdempotencyKey::generate(), project.task).await;

  let Err(Rejection::RunNotStarted {
    run: id,
    why: WorktreeFailure::Git(said),
  }) = refused
  else {
    panic!("git's refusal is the reason, got {refused:?}");
  };
  assert!(said.as_str().contains("https://***@example.com/lfs"), "{said:?}");
  assert!(!said.as_str().contains("hunter2"), "no credentials: {said:?}");
  let run = stored(root, id);
  assert_eq!(run.own_branch(), None, "it made no branch");
  assert!(!Worktrees::new(root).expect("worktrees").path(id).exists());
  assert_eq!(
    git(root, &["branch", "--list", &format!("{FIRST_RUN_BRANCH}-2")]),
    "",
    "the branch made for it went with its worktree"
  );
  let (runner, program) = (ProcessRunner::new(Duration::from_secs(60)), Path::new("git"));
  let left = leftovers(&runner, program, root, &run).expect("leftovers");
  assert!(left.worktrees.is_empty() && left.branches.is_empty(), "{left:?}");
  let cleanup = clean_up(&runner, program, root, &run, None).expect("cleanup");
  assert!(
    cleanup.deleted.is_empty() && cleanup.kept_branches.is_empty(),
    "{cleanup:?}"
  );
  assert_eq!(
    git(root, &["branch", "--list", FIRST_RUN_BRANCH]),
    FIRST_RUN_BRANCH,
    "the taken branch stays"
  );
  let db = ProjectDb::open(root).expect("open");
  assert!(db.run_branches().expect("run branches").is_empty());
}
