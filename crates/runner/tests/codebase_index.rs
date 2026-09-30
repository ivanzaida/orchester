//! The codebase index of a run's base revision, end to end against `orchester-fake-codex`: built
//! before the first step, removed with the worktree that used it, and reported to the agent when it
//! cannot be built while the run goes on without it. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test codebase_index`.

mod support;

use std::{fs, path::Path, time::Duration};

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::{IdempotencyKey, RunId},
  integration::{Leftovers, leftovers},
  run::{Run, RunStatus},
  store::{ActivityStore, RunStore},
};
use orchester_exec::ProcessRunner;
use orchester_store::ProjectDb;

use self::support::{Setup, git, project, scheduler, wait_for};

fn index_of(root: &Path, run: &Run) -> std::path::PathBuf {
  root.join(".orchester/index").join(run.base().as_str())
}

fn left_by(root: &Path, run: &Run) -> Leftovers {
  let git = ProcessRunner::new(Duration::from_secs(30));
  leftovers(&git, Path::new("git"), root, run).expect("what the run left")
}

fn index_problems(root: &Path, run: RunId) -> Vec<String> {
  ProjectDb::open(root)
    .expect("open")
    .activity(run, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::RunnerProblem {
        problem: RunnerProblem::CodebaseIndexUnavailable { detail },
      } => Some(detail),
      _ => None,
    })
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn codebase_index_is_built_for_the_base_revision_before_the_first_step() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let ended = wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Succeeded).await;

  let index = index_of(root, &ended);
  assert_eq!(
    fs::read_to_string(index.join("tree.txt")).expect("the tree"),
    "README.md\tmarkdown\t1\n"
  );
  let manifest = fs::read_to_string(index.join("manifest.json")).expect("the manifest");
  assert!(manifest.contains(ended.base().as_str()), "{manifest}");
  assert!(index_problems(root, run).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn codebase_index_goes_with_the_worktree_removed_before_running_again() {
  let project = project(&Setup::new("Please fail."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let first = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let failed = wait_for(root, &mut events, first, |run| run.status().is_terminal()).await;
  let first_index = index_of(root, &failed);
  assert!(first_index.is_dir());
  fs::write(root.join("README.md"), "changed\n").expect("change the checkout");
  git(root, &["commit", "--quiet", "--all", "-m", "change"]);

  let again = scheduler
    .run_again(
      IdempotencyKey::generate(),
      project.task,
      first,
      Some(left_by(root, &failed)),
    )
    .await
    .expect("the task runs again");

  assert!(!first_index.exists(), "the index went with the worktree that used it");
  let next = wait_for(root, &mut events, again, |run| run.status().is_terminal()).await;
  assert_ne!(next.base(), failed.base());
  assert!(index_of(root, &next).is_dir());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unavailable_codebase_index_is_reported_to_the_agent_and_the_run_goes_on() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  // A file where the index folder belongs: nothing can be built there.
  fs::write(root.join(".orchester/index"), "not a folder\n").expect("block the index folder");
  let (scheduler, mut events) = scheduler(root);
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Succeeded).await;

  let problems = index_problems(root, run);
  assert_eq!(problems.len(), 1, "{problems:?}");
  assert!(problems[0].contains("index"), "{problems:?}");
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(db.run(run).expect("read").expect("run").status(), &RunStatus::Succeeded);
}
