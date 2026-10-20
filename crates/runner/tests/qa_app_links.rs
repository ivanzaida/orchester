//! The cleanup before a QA app starts never follows a link: a QA folder's `bin` that is a junction to a folder outside
//! the worktree, in this run's worktree or named by a forged slot file, has nothing stopped through it, so a program
//! running from that outside folder (standing in for the person's installed app) keeps running.
//! Windows only, as `qa_app`: the stand-in script (`support::qa`) is Windows PowerShell, and junctions are Windows'.
//! Needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test qa_app_links qa_app`.
#![cfg(windows)]

mod support;

use std::{
  fs,
  path::Path,
  process::{Child, Command, Stdio},
};

use orchester_core::{id::IdempotencyKey, run::RunStatus};

use self::support::{
  is_running,
  qa::{approve, qa_project, verify_step},
  scheduler, wait_for,
};

const TEST_COMMAND: &str = env!("CARGO_BIN_EXE_orchester-test-command");

/// A program running from `folder`, outside every worktree, that the test ends itself.
struct Outside(Child);

impl Outside {
  fn start(folder: &Path) -> Self {
    let program = folder.join("orchester-app.exe");
    fs::copy(TEST_COMMAND, &program).expect("copy");
    let mut command = Command::new(&program);
    command
      .args(["sleep", "600"])
      .stdout(Stdio::null())
      .stderr(Stdio::null());
    Self(command.spawn().expect("start the outside program"))
  }

  fn pid(&self) -> String {
    self.0.id().to_string()
  }
}

impl Drop for Outside {
  fn drop(&mut self) {
    // The program may have ended already; ending and reaping it is only the test's cleanup.
    self.0.kill().ok();
    self.0.wait().ok();
  }
}

/// Makes `link` a junction to `target`.
fn junction(link: &Path, target: &Path) {
  fs::create_dir_all(link.parent().expect("a parent")).expect("the link's folder");
  let made = Command::new("cmd")
    .args(["/C", "mklink", "/J"])
    // cmd reads a `/` as a switch.
    .arg(link.to_string_lossy().replace('/', "\\"))
    .arg(target.to_string_lossy().replace('/', "\\"))
    .output()
    .expect("mklink");
  assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
}

#[tokio::test(flavor = "multi_thread")]
async fn qa_app_cleanup_never_stops_a_program_through_a_linked_bin() {
  let outside = tempfile::tempdir().expect("an outside folder");
  let person = Outside::start(outside.path());
  let project = qa_project(verify_step("Start the QA app."));
  let root = project.repo.path();
  // A forged slot record naming a QA folder of a worktree, its `bin` a junction to the outside folder.
  let forged = root.join(".orchester/worktrees/forged/.tmp/qa/run-9-verify");
  fs::create_dir_all(&forged).expect("the forged QA folder");
  junction(&forged.join("bin"), outside.path());
  let slots = root.join(".orchester").join("qa-slots");
  fs::create_dir_all(&slots).expect("slots");
  let forged = fs::canonicalize(&forged).expect("canonical");
  fs::write(
    slots.join("slot-0.holder"),
    format!("0\nORC-9\n9\n{}", forged.display()),
  )
  .expect("forged record");

  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  // While the start waits for its approval, the agent's worktree gets a `bin` junction to the outside folder.
  let own = root
    .join(".orchester/worktrees")
    .join(id.to_string())
    .join(".tmp/qa/run-1-verify/bin");
  junction(&own, outside.path());
  approve(&scheduler, root, &mut events, id).await;
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert!(is_running(&person.pid()), "nothing outside the worktree was stopped");
}
