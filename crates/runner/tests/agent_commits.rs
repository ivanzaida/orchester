//! Agents that commit their work themselves (ORC-147): the step records the commit as its checkpoint, so whatever
//! reads a run's work through its checkpoints sees it; and succeeded runs from before, which recorded none, get it
//! when the runner starts.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test agent_commits agent_commits`.

mod support;

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::IdempotencyKey,
  run::{Run, RunStatus},
  store::{ActivityStore, RunStore},
};
use orchester_store::ProjectDb;

use self::support::{Setup, TestProject, git, project, scheduler, settled, wait_for};

async fn run_to_the_end(brief: &'static str) -> (TestProject, Run) {
  let project = project(&Setup::new(brief));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, settled).await;
  (project, run)
}

fn run_worktree(root: &Path, run: &Run) -> PathBuf {
  root.join(".orchester").join("worktrees").join(run.id().to_string())
}

/// The files the run changed up to its last checkpoint, as its Changes read them.
fn checkpointed_files(root: &Path, run: &Run) -> String {
  let checkpoint = run.last_checkpoint().expect("a checkpoint");
  git(
    root,
    &[
      "diff",
      "--name-only",
      run.base().as_str(),
      checkpoint.revision().as_str(),
    ],
  )
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_commits_of_its_own_with_a_clean_tree_are_the_steps_checkpoint() {
  let (project, run) = run_to_the_end("Commit the work.").await;
  let root = project.repo.path();
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let branch = run.branch().as_str();
  let head = git(root, &["rev-parse", branch]);
  assert_eq!(
    run.last_checkpoint().map(|checkpoint| checkpoint.revision().as_str()),
    Some(head.as_str())
  );
  // The agent's commit is the step's work: the runner added none.
  assert_eq!(
    git(root, &["log", "-1", "--format=%s", branch]),
    "The agent's own commit"
  );
  assert_eq!(checkpointed_files(root, &run), "README.md");
  assert_eq!(git(&run_worktree(root, &run), &["status", "--porcelain"]), "");
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_commits_with_changes_left_get_one_runner_commit_on_top() {
  let (project, run) = run_to_the_end("Commit, then draft.").await;
  let root = project.repo.path();
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let branch = run.branch().as_str();
  let head = git(root, &["rev-parse", branch]);
  assert_eq!(
    run.last_checkpoint().map(|checkpoint| checkpoint.revision().as_str()),
    Some(head.as_str())
  );
  let subjects = git(root, &["log", "-2", "--format=%s", branch]);
  let subjects: Vec<&str> = subjects.lines().collect();
  assert!(subjects[0].starts_with("ORC-1: Implement"), "{subjects:?}");
  assert_eq!(subjects[1], "The agent's own commit");
  assert_eq!(git(root, &["show", &format!("{branch}:README.md")]), "hi\nhello\ndraft");
  assert_eq!(checkpointed_files(root, &run), "README.md");
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_commits_none_when_it_changes_nothing() {
  let (project, run) = run_to_the_end("Change nothing.").await;
  let root = project.repo.path();
  assert!(run.last_checkpoint().is_none(), "{:?}", run.last_checkpoint());
  assert_eq!(git(root, &["rev-parse", run.branch().as_str()]), run.base().as_str());
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_commits_off_the_runs_branch_are_not_taken_and_reported() {
  let (project, run) = run_to_the_end("Commit on a side branch.").await;
  let root = project.repo.path();
  assert!(run.last_checkpoint().is_none(), "{:?}", run.last_checkpoint());
  let db = ProjectDb::open(root).expect("open");
  let reported = db
    .activity(run.id(), 0)
    .expect("activity")
    .into_iter()
    .find_map(|entry| match entry.event {
      AgentEvent::RunnerProblem {
        problem: RunnerProblem::CheckpointFailed { detail },
      } => Some(detail),
      _ => None,
    })
    .expect("the commits off the branch are reported");
  assert!(reported.contains("agent-side"), "{reported}");
  assert!(reported.contains(run.branch().as_str()), "{reported}");
}

#[tokio::test(flavor = "multi_thread")]
async fn agent_commits_of_a_succeeded_run_without_a_checkpoint_are_recorded_when_the_runner_starts() {
  let (project, run) = run_to_the_end("Change nothing.").await;
  let root = project.repo.path();
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let stored = |root: &Path| {
    ProjectDb::open(root)
      .expect("open")
      .run(run.id())
      .expect("read")
      .expect("the run")
  };
  // Its branch is where it started: a runner that starts records nothing.
  drop(scheduler(root));
  assert!(stored(root).last_checkpoint().is_none());

  // As an agent's own commit left it under a runner before ORC-147.
  let worktree = run_worktree(root, &run);
  std::fs::write(worktree.join("notes.md"), "notes\n").expect("write");
  git(&worktree, &["add", "notes.md"]);
  git(&worktree, &["commit", "--quiet", "-m", "The agent's own commit"]);
  let tip = git(root, &["rev-parse", run.branch().as_str()]);

  drop(scheduler(root));
  let repaired = stored(root);
  assert_eq!(repaired.status(), &RunStatus::Succeeded);
  assert_eq!(
    repaired
      .last_checkpoint()
      .map(|checkpoint| checkpoint.revision().as_str()),
    Some(tip.as_str())
  );
  assert_eq!(checkpointed_files(root, &repaired), "notes.md");
  // Once: a runner that starts again records no other.
  let events = ProjectDb::open(root)
    .expect("open")
    .events(run.id())
    .expect("events")
    .len();
  drop(scheduler(root));
  assert_eq!(
    ProjectDb::open(root)
      .expect("open")
      .events(run.id())
      .expect("events")
      .len(),
    events
  );
}
