//! The merge queue's work in a run's worktree, carried out by the real `orchester-runner` (ORC-684,
//! `Command::MergeQueue`): it merges the base branch into the run's branch in its worktree, and runs the checking tools
//! there in order until one does not pass, answering with the end of what that one printed.
//! `cargo test -p orchester-runner --test merge_queue merge_queue`.

use std::{fs, path::Path, process::Command as Git, time::Duration};

use orchester_core::{
  id::{ProjectId, RunId},
  integration::delivery::Synced,
  merge_queue::recheck::{RecheckTool, Rechecked},
  name::Name,
  project::{Project, ProjectKey},
  resource::{CommandAction, CommandProgram},
  revision::Revision,
  run::BranchName,
};
use orchester_ipc::{
  Command, Event, Reply, Request, RequestId, RunnerClient,
  merge_queue::{MergeQueueCommand, MergeQueueReply},
};
use orchester_store::ProjectDb;
use tempfile::TempDir;
use tokio::time::timeout;

const RUNNER: &str = env!("CARGO_BIN_EXE_orchester-runner");
const PATIENCE: Duration = Duration::from_secs(60);

fn git(dir: &Path, args: &[&str]) -> String {
  let output = Git::new("git").args(args).current_dir(dir).output().expect("git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout).expect("text").trim().to_owned()
}

/// A project on `main` with run `run`'s worktree on its own branch, one commit ahead, while `main` moved on.
fn project(run: RunId) -> (TempDir, std::path::PathBuf) {
  let repo = tempfile::tempdir().expect("repository");
  let root = repo.path();
  git(root, &["init", "-b", "main"]);
  git(root, &["config", "user.name", "Merge queue test"]);
  git(root, &["config", "user.email", "test@example.invalid"]);
  git(root, &["config", "core.autocrlf", "false"]);
  fs::write(root.join(".gitignore"), ".orchester/\n").expect("ignore");
  fs::write(root.join("shared.txt"), "base\n").expect("shared");
  git(root, &["add", "."]);
  git(root, &["commit", "-m", "base"]);
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    Name::new("Kontur").expect("name"),
  );
  drop(ProjectDb::create(root, project).expect("project"));
  let worktree = root.join(".orchester/worktrees").join(run.to_string());
  let path = worktree.to_str().expect("path").to_owned();
  git(root, &["worktree", "add", "-b", "orchester/orc-1-run-1", &path, "main"]);
  fs::create_dir_all(worktree.join("tools")).expect("tools");
  for (name, code) in [("pass", 0), ("fail", 3)] {
    write_script(&worktree.join("tools"), name, code);
  }
  git(&worktree, &["add", "-A"]);
  git(&worktree, &["commit", "-m", "ORC-1: Implement"]);
  fs::write(root.join("other.txt"), "main moved\n").expect("other");
  git(root, &["add", "."]);
  git(root, &["commit", "-m", "A change on main"]);
  (repo, worktree)
}

/// A script `name` that prints a line on each output and exits with `code`.
#[cfg(windows)]
fn write_script(folder: &Path, name: &str, code: i32) {
  let script = format!("@echo off\r\necho {name} said this\r\n1>&2 echo {name} complained\r\nexit /b {code}\r\n");
  fs::write(folder.join(format!("{name}.cmd")), script).expect("script");
}

#[cfg(unix)]
fn write_script(folder: &Path, name: &str, code: i32) {
  use std::os::unix::fs::PermissionsExt;
  let file = folder.join(format!("{name}.cmd"));
  let script = format!("#!/bin/sh\necho {name} said this\necho {name} complained 1>&2\nexit {code}\n");
  fs::write(&file, script).expect("script");
  fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).expect("executable");
}

fn tool(name: &str, script: &str) -> RecheckTool {
  RecheckTool {
    name: name.to_owned(),
    action: CommandAction::new(
      CommandProgram::new(&format!("tools/{script}.cmd")).expect("program"),
      Vec::new(),
    ),
  }
}

async fn ask(client: &mut RunnerClient, id: u64, command: MergeQueueCommand) -> MergeQueueReply {
  let request = Request {
    id: RequestId::new(id),
    command: Command::MergeQueue(command),
  };
  client.send(&request).await.expect("send");
  loop {
    let event = timeout(PATIENCE, client.next_event())
      .await
      .expect("reply in time")
      .expect("read event")
      .expect("an event");
    match event {
      Event::Reply {
        id: answered,
        reply: Reply::MergeQueue(reply),
      } if answered == RequestId::new(id) => return reply,
      Event::Reply { id: answered, reply } if answered == RequestId::new(id) => panic!("refused: {reply:?}"),
      _ => {}
    }
  }
}

#[tokio::test]
async fn merge_queue_runner_syncs_the_base_in_and_rechecks_in_the_runs_worktree() {
  let run = RunId::generate();
  let (repo, worktree) = project(run);
  let main = Revision::new(&git(repo.path(), &["rev-parse", "main"])).expect("revision");
  let mut client = timeout(PATIENCE, RunnerClient::launch(Path::new(RUNNER), repo.path(), None))
    .await
    .expect("launch in time")
    .expect("launch");

  let synced = ask(
    &mut client,
    1,
    MergeQueueCommand::SyncBase {
      run,
      base: BranchName::new("main").expect("branch"),
      branch: BranchName::new("orchester/orc-1-run-1").expect("branch"),
    },
  )
  .await;
  let tip = Revision::new(&git(&worktree, &["rev-parse", "HEAD"])).expect("revision");
  assert_eq!(synced, MergeQueueReply::Synced(Synced::Clean { base: main, tip }));
  assert!(worktree.join("other.txt").is_file(), "main's change is in the worktree");

  let passed = ask(
    &mut client,
    2,
    MergeQueueCommand::Recheck {
      run,
      tools: vec![tool("Run tests", "pass")],
    },
  )
  .await;
  assert_eq!(
    passed,
    MergeQueueReply::Rechecked(Rechecked::Passed {
      tools: "Run tests".to_owned()
    })
  );

  let failed = ask(
    &mut client,
    3,
    MergeQueueCommand::Recheck {
      run,
      tools: vec![tool("Run tests", "pass"), tool("Lint", "fail"), tool("Never", "pass")],
    },
  )
  .await;
  assert_eq!(
    failed,
    MergeQueueReply::Rechecked(Rechecked::Failed {
      tool: "Lint".to_owned(),
      detail: "fail said this\nfail complained".to_owned(),
    })
  );
}
