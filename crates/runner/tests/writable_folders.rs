//! Extra writable folders end to end against `orchester-fake-codex` and `orchester-fake-claude`:
//! the folders the Builder profile allows reach the CLI's arguments, resolved in the repository,
//! and one under `.orchester/` is created first; a folder that can no longer be given stops the
//! agent before its CLI starts. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test writable_folders writable_folders`.

mod support;

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::{
    AgentProvider,
    stream::{AgentEvent, InvocationOutcome, RunnerProblem},
  },
  id::{IdempotencyKey, RunId},
  permission::{FolderBounds, WritableFolder, WritableFolders},
  resource::{AgentProfile, Draft, ResourceKind},
  run::{Run, RunStatus, Stuck},
  store::{ActivityStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use tempfile::TempDir;

use self::support::{Setup, project, reported, scheduler, settled, threads, wait_for};

/// Publishes the Builder profile again, allowing it to write to `entries` besides its worktree.
fn allow_folders(root: &Path, entries: &[String]) {
  let mut db = ProjectDb::open(root).expect("open");
  let builder = db.resources(ResourceKind::AgentProfile).expect("profiles")[0].id();
  let latest = db
    .latest_version::<AgentProfile>(builder)
    .expect("read")
    .expect("published");
  let bounds = FolderBounds::new(root, None).expect("bounds");
  let folders = entries
    .iter()
    .map(|entry| WritableFolder::new(entry, &bounds).expect("a folder the agent may write to"))
    .collect();
  let folders = WritableFolders::new(folders).expect("unique");
  let permissions = latest.body().permissions().clone().with_writable_folders(folders);
  let body = latest.body().clone().with_permissions(permissions);
  let now = Timestamp::now();
  db.save_draft(builder, &Draft::new(body, Some(latest.number()), now))
    .expect("draft");
  db.publish_draft::<AgentProfile>(builder, now).expect("publish");
}

/// Runs the project's task to its end.
async fn run(root: &Path, task: orchester_core::id::TaskId) -> (RunId, Run) {
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, settled).await;
  (id, run)
}

/// The repository's `.orchester/target` and a folder beside the repository, as the profile lists
/// them and as the CLI gets them (links resolved).
fn folders(root: &Path, outside: &TempDir) -> (Vec<String>, Vec<PathBuf>) {
  let entries = vec![
    ".orchester/target".to_owned(),
    outside.path().to_str().expect("unicode").to_owned(),
  ];
  let plain = |path: PathBuf| PathBuf::from(path.to_str().expect("unicode").trim_start_matches(r"\\?\"));
  let repository = plain(std::fs::canonicalize(root).expect("canonical"));
  let outside = plain(std::fs::canonicalize(outside.path()).expect("canonical"));
  (entries, vec![repository.join(".orchester").join("target"), outside])
}

fn toml(path: &Path) -> String {
  let text = path
    .to_str()
    .expect("unicode")
    .replace('\\', r"\\")
    .replace('"', "\\\"");
  format!("\"{text}\"")
}

#[tokio::test(flavor = "multi_thread")]
async fn writable_folders_reach_codex_as_writable_roots_and_are_created() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let outside = tempfile::tempdir().expect("a folder beside the repository");
  let (entries, paths) = folders(root, &outside);
  allow_folders(root, &entries);
  let (id, run) = run(root, project.task).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let argv = reported(&db, id, "argv: ");
  // After the worktree, which App Server reports as its environment's workspace root.
  let roots = format!(",{},{}]", toml(&paths[0]), toml(&paths[1]));
  assert!(argv[0].contains(&roots), "{roots} in {argv:?}");
  assert_eq!(threads(&db, id)[0]["sandbox"], "workspace-write");
  assert!(paths[0].is_dir(), "created before the agent started");
}

#[tokio::test(flavor = "multi_thread")]
async fn writable_folders_reach_claude_as_one_add_dir_each() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Say hello.")
  });
  let root = project.repo.path();
  let outside = tempfile::tempdir().expect("a folder beside the repository");
  let (entries, paths) = folders(root, &outside);
  allow_folders(root, &entries);
  let (id, run) = run(root, project.task).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let argv = reported(&ProjectDb::open(root).expect("open"), id, "argv: ");
  let add = format!("--add-dir {} --add-dir {}", paths[0].display(), paths[1].display());
  assert!(argv[0].contains(&add), "{add} in {argv:?}");
  // With allowed commands the step edits by rule, one for the worktree and one for each folder.
  assert!(argv[0].contains("--permission-mode default"), "{argv:?}");
  assert_eq!(argv[0].matches(" Edit(/").count(), 3, "{argv:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn writable_folders_that_are_gone_stop_the_agent_before_its_cli_starts() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let outside = tempfile::tempdir().expect("a folder beside the repository");
  let entry = outside.path().to_str().expect("unicode").to_owned();
  allow_folders(root, std::slice::from_ref(&entry));
  outside.close().expect("the folder is removed");
  let (id, run) = run(root, project.task).await;
  assert!(
    matches!(run.status(), RunStatus::WaitingForPerson(Stuck::BrokeAgain(_))),
    "{:?}",
    run.status()
  );
  let db = ProjectDb::open(root).expect("open");
  assert!(reported(&db, id, "argv: ").is_empty(), "the CLI never started");
  assert_eq!(
    db.invocations(id).expect("invocations")[0].outcome,
    Some(InvocationOutcome::NotStarted)
  );
  let detail = db
    .activity(id, 0)
    .expect("activity")
    .into_iter()
    .find_map(|entry| match entry.event {
      AgentEvent::RunnerProblem {
        problem: RunnerProblem::CouldNotStart { detail },
      } => Some(detail),
      _ => None,
    })
    .expect("the activity says why");
  assert!(detail.contains(&entry) && detail.contains("does not exist"), "{detail}");
}
