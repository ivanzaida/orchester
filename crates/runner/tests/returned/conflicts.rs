//! *Ask an agent to resolve* (`continue_run`'s `resolve_conflicts`, the Deliver step's Integrate step): the work of a
//! run whose merge into its base branch conflicted goes back to its writing step, and before that step's agent starts
//! the runner merges the base branch into its worktree, as a sandboxed Codex agent cannot write the repository's git
//! data. The agent, `orchester-fake-codex`, only edits away the conflicts its prompt lists, and the step's save
//! commits the merge with its work; a clean merge is committed at once; markers left fail the step, nothing of them
//! committed, and the merge stays in progress for a person.

use std::path::Path;

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::{IdempotencyKey, RunId},
  run::{Answerer, BranchName, MergeConflicts, ReviewNote, Run, RunStatus},
  store::ActivityStore,
};
use orchester_ipc::WorkReturn;
use orchester_store::ProjectDb;

use super::{
  implement, read,
  support::{Setup, TestProject, git, project, reported, scheduler, wait_for, wait_for_idle},
};

/// The note *Ask an agent to resolve* sends (`return_work::conflict_note`).
const RESOLVE: &str = "Resolve the conflicts with main listed below, keeping both sides' intent.";

/// `run`'s worktree in the repository at `root`.
fn worktree(root: &Path, run: RunId) -> std::path::PathBuf {
  root.join(".orchester").join("worktrees").join(run.to_string())
}

/// A return of `run`, as stored now, to Implement, to resolve the conflicts of its merge into `main` in `files`.
fn resolving(root: &Path, run: RunId, files: &[&str]) -> WorkReturn {
  WorkReturn {
    run,
    step: implement(),
    note: ReviewNote::new(RESOLVE).expect("a note"),
    conflicts: Some(MergeConflicts {
      base: BranchName::new("main").expect("a branch"),
      files: files.iter().map(|file| (*file).to_owned()).collect(),
    }),
    profile: None,
    by: Answerer::Person,
    expected: read(root, run).last_sequence(),
  }
}

/// A project whose run succeeded — its agent appended `hello` to `README.md` — after which `land` changed `main`, and
/// whose work was then returned to resolve the conflicts in `files`; the run once that return ended or waits for a
/// person, and `main`'s head.
async fn returned_after(land: impl FnOnce(&Path), files: &[&str]) -> (TestProject, Run, String) {
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
  land(root);
  let head = git(root, &["rev-parse", "main"]);
  scheduler
    .return_ended(resolving(root, id, files))
    .await
    .expect("returned");
  let ended = wait_for(root, &mut events, id, |run| {
    run.attempts().len() >= 2 && (run.status().is_terminal() || matches!(run.status(), RunStatus::WaitingForPerson(_)))
  })
  .await;
  wait_for_idle(&scheduler).await;
  (project, ended, head)
}

/// Rewords `README.md` on `main`, as the change the run's merge conflicted with.
fn reword_readme(root: &Path) {
  std::fs::write(root.join("README.md"), "hi\nfrom the base\n").expect("written");
  git(root, &["commit", "--quiet", "-am", "ORC-7: Reword the readme"]);
}

/// The merge commits on `branch` with their parents, one per line.
fn merges(root: &Path, branch: &str) -> String {
  git(root, &["rev-list", "--merges", "--parents", branch])
}

#[tokio::test(flavor = "multi_thread")]
async fn returned_conflicts_are_merged_by_the_runner_and_edited_away_by_the_codex_agent() {
  let (project, run, head) = returned_after(reword_readme, &["README.md"]).await;
  let root = project.repo.path();
  assert_eq!(run.status(), &RunStatus::Succeeded);

  let prompt = reported(&ProjectDb::open(root).expect("open"), run.id(), "prompt| ");
  for line in [
    "## The merge in this worktree",
    "Orchester merged `main` into this worktree before this step started: do not merge or commit it yourself.",
    "- `README.md`",
  ] {
    assert!(prompt.iter().any(|said| said == line), "{line} in {prompt:?}");
  }
  assert!(
    prompt
      .iter()
      .any(|said| said.starts_with("Git stopped at conflicts in these files")),
    "{prompt:?}"
  );

  let branch = run.branch().as_str();
  assert!(merges(root, branch).contains(&head), "the base was merged in");
  let committed = git(root, &["show", &format!("{branch}:README.md")]);
  assert!(!committed.contains("<<<<<<<"), "{committed}");
  assert!(committed.contains("from the base"), "{committed}");
  assert!(committed.contains("hello"), "{committed}");
  let worktree = worktree(root, run.id());
  assert_eq!(
    git(&worktree, &["status", "--porcelain"]),
    "",
    "the merge is committed, nothing left over"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn returned_conflicts_that_merge_cleanly_now_are_committed_before_the_agent_starts() {
  let land = |root: &Path| {
    std::fs::write(root.join("landed.md"), "landed\n").expect("written");
    git(root, &["add", "landed.md"]);
    git(root, &["commit", "--quiet", "-m", "ORC-7: Land a file"]);
  };
  let (project, run, head) = returned_after(land, &["README.md"]).await;
  let root = project.repo.path();
  assert_eq!(run.status(), &RunStatus::Succeeded);

  let prompt = reported(&ProjectDb::open(root).expect("open"), run.id(), "prompt| ");
  assert!(
    prompt
      .iter()
      .any(|said| said.starts_with("Git merged `main` without conflicts, as merge commit ")),
    "{prompt:?}"
  );
  assert!(
    !prompt.iter().any(|said| said.starts_with("Git stopped at conflicts")),
    "{prompt:?}"
  );
  let branch = run.branch().as_str();
  assert!(merges(root, branch).contains(&head), "the base was merged in");
  assert_eq!(
    git(root, &["show", &format!("{branch}:landed.md")]),
    "landed",
    "what landed is on the run's branch"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn returned_conflict_markers_left_fail_the_step_and_are_never_committed() {
  let leave = |root: &Path| {
    reword_readme(root);
    std::fs::write(root.join(".orchester").join("fake-leave-conflicts"), "left").expect("written");
  };
  let (project, run, head) = returned_after(leave, &["README.md"]).await;
  let root = project.repo.path();
  // The step broke, and broke again in the attempt that restarted it: the person decides.
  assert!(
    matches!(run.status(), RunStatus::WaitingForPerson(_)),
    "{:?}",
    run.status()
  );

  let detail = ProjectDb::open(root)
    .expect("open")
    .activity(run.id(), 0)
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
  assert!(!merges(root, branch).contains(&head), "the merge was not committed");
  let worktree = worktree(root, run.id());
  assert_eq!(git(&worktree, &["rev-parse", "MERGE_HEAD"]), head, "still in progress");
}
