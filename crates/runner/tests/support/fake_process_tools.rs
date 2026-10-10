//! The fake CLIs start a stand-in and its child, then stop them through the invocation's relay.

use std::{
  fs,
  io::{BufRead, BufReader},
  path::Path,
  process::{Child, Command, Stdio},
};

use serde_json::{Value, json};

struct Started {
  child: Child,
  descendant: u32,
}

impl Drop for Started {
  fn drop(&mut self) {
    // On a failing test only: these handles/PIDs belong to stand-ins this helper started.
    match self.child.try_wait() {
      Ok(Some(_)) => {}
      Ok(None) => {
        if let Err(error) = self.child.kill() {
          eprintln!("cannot clean up stand-in: {error}");
        }
        if let Err(error) = self.child.wait() {
          eprintln!("cannot reap stand-in: {error}");
        }
      }
      Err(error) => eprintln!("cannot inspect stand-in: {error}"),
    }
    let command =
      orchester_exec::run_processes::RunProcesses::new(&std::env::current_dir().expect("worktree")).expect("scope");
    let call = orchester_core::orchester_tools::ProcessCall::Stop {
      pid: std::num::NonZeroU32::new(self.descendant).expect("child pid"),
    };
    let result = command.run(call);
    if result.is_error() {
      eprintln!("cleanup: {}", result.text());
    }
  }
}

fn start(program: &Path) -> Started {
  let mut command = Command::new(program);
  command
    .args(["spawn-sleeper", "sleep", "600"])
    .env("ORCHESTER_TEST_COMMAND_DETACHED", "1")
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
  #[cfg(windows)]
  {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0000_0008);
  }
  let mut child = command.spawn().expect("stand-in");
  let mut reader = BufReader::new(child.stdout.take().expect("stdout"));
  let mut line = String::new();
  reader.read_line(&mut line).expect("pid");
  line.clear();
  reader.read_line(&mut line).expect("child pid");
  let descendant = line
    .trim()
    .strip_prefix("child pid: ")
    .expect("child pid")
    .parse()
    .expect("pid");
  Started { child, descendant }
}

pub fn exercise(worktree: &Path, mut call: impl FnMut(&str, &Value)) {
  let original = std::env::current_exe()
    .expect("CLI")
    .parent()
    .expect("bin")
    .join(if cfg!(windows) {
      "orchester-test-command.exe"
    } else {
      "orchester-test-command"
    });
  let folder = worktree.join(".tmp/process-tools");
  fs::create_dir_all(&folder).expect("scratch");
  let program = folder.join(if cfg!(windows) { "stand-in.exe" } else { "stand-in" });
  fs::copy(original, &program).expect("copy");
  let mut app = start(&program);
  let pid = app.child.id();
  eprintln!("process-stand-in: {pid} child {}", app.descendant);
  call("list_run_processes", &json!({}));
  call("stop_run_process", &json!({"pid":std::process::id()}));
  call("stop_run_process", &json!({"pid":pid}));
  assert!(app.child.try_wait().expect("reap").is_some(), "stand-in was stopped");
  call("stop_run_process", &json!({"pid":pid}));
  eprintln!("process-stand-in: stopped");
}
