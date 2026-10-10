//! Starts copies of this test binary as sleepers; all scratch and all processes belong to this test.

#[cfg(windows)]
#[path = "support/process_window.rs"]
mod process_window;

use std::{
  fs,
  num::NonZeroU32,
  path::{Path, PathBuf},
  process::{Child, Command, Stdio},
  time::Duration,
};

use orchester_core::orchester_tools::ProcessCall;
use orchester_exec::run_processes::RunProcesses;
use serde_json::Value;

struct Sleeper(Child);

impl Sleeper {
  fn start(program: &Path) -> Self {
    Self::configured(program, false, None)
  }

  fn configured(program: &Path, window: bool, child_program: Option<&Path>) -> Self {
    let ready = program.with_extension("ready");
    let mut command = Command::new(program);
    command.env("ORCHESTER_PROCESS_READY", &ready);
    if let Some(child) = child_program {
      command.env("ORCHESTER_PROCESS_CHILD", child);
    }
    if window {
      command.env("ORCHESTER_PROCESS_WINDOW", "1");
    }
    command
      .args(["--exact", "run_processes_stand_in", "--ignored"])
      .env("ORCHESTER_PROCESS_STAND_IN", "1")
      .stdout(Stdio::null())
      .stderr(Stdio::null());
    #[cfg(windows)]
    {
      use std::os::windows::process::CommandExt;
      command.creation_flags(0x0000_0008);
    }
    let child = command.spawn().expect("stand-in");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !ready.exists() {
      assert!(std::time::Instant::now() < deadline, "stand-in did not start");
      std::thread::sleep(Duration::from_millis(10));
    }
    Self(child)
  }

  fn call(&self) -> ProcessCall {
    ProcessCall::Stop {
      pid: NonZeroU32::new(self.0.id()).expect("pid"),
    }
  }
}

impl Drop for Sleeper {
  fn drop(&mut self) {
    match self.0.try_wait() {
      Ok(None) => {
        if let Err(error) = self.0.kill() {
          panic!("cannot clean up started child: {error}");
        }
        self.0.wait().expect("reap");
      }
      Ok(Some(_)) => {}
      Err(error) => panic!("cannot inspect started child: {error}"),
    }
  }
}

#[test]
#[ignore = "spawned by the process tests"]
fn run_processes_stand_in() {
  assert!(
    std::env::var_os("ORCHESTER_PROCESS_STAND_IN").is_some(),
    "only a copied child sleeps"
  );
  #[cfg(windows)]
  if std::env::var_os("ORCHESTER_PROCESS_WINDOW").is_some() {
    process_window::wait_for_close().expect("window");
    return;
  }
  let child = std::env::var_os("ORCHESTER_PROCESS_CHILD").map(|program| {
    let child_ready = PathBuf::from(&program).with_extension("child-ready");
    let mut command = Command::new(program);
    command.env("ORCHESTER_PROCESS_READY", &child_ready);
    command
      .args(["--exact", "run_processes_stand_in", "--ignored"])
      .env_remove("ORCHESTER_PROCESS_CHILD")
      .stdout(Stdio::null())
      .stderr(Stdio::null());
    #[cfg(windows)]
    {
      use std::os::windows::process::CommandExt;
      command.creation_flags(0x0000_0008);
    }
    let child = command.spawn().expect("child");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !child_ready.exists() {
      assert!(std::time::Instant::now() < deadline, "child did not start");
      std::thread::sleep(Duration::from_millis(10));
    }
    child
  });
  fs::write(
    std::env::var_os("ORCHESTER_PROCESS_READY").expect("ready path"),
    child
      .as_ref()
      .map_or_else(|| "ready".to_owned(), |child| child.id().to_string()),
  )
  .expect("ready");
  std::thread::sleep(Duration::from_secs(60));
}

fn folder() -> tempfile::TempDir {
  let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/run-processes-exec");
  fs::create_dir_all(&base).expect("scratch");
  tempfile::tempdir_in(base).expect("folder")
}

fn copy(root: &Path, name: &str) -> PathBuf {
  let destination = root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
  fs::copy(std::env::current_exe().expect("test binary"), &destination).expect("copy");
  destination
}

#[test]
fn run_processes_lists_exact_scope_and_refuses_outside_and_protected_programs() {
  let folder = folder();
  let worktree = folder.path().join("run");
  let other = folder.path().join("other-run");
  fs::create_dir(&worktree).expect("run");
  fs::create_dir(&other).expect("other");
  let inside = copy(&worktree, "inside");
  let outside = copy(&other, "outside");
  let mut own = Sleeper::start(&inside);
  let mut foreign = Sleeper::start(&outside);
  let executor = RunProcesses::new(&worktree).expect("scope");
  let result = executor.run(ProcessCall::List);
  assert!(!result.is_error(), "{}", result.text());
  let listed: Vec<Value> = serde_json::from_str(result.text()).expect("list");
  assert_eq!(listed.len(), 1, "{listed:?}");
  assert_eq!(listed[0]["pid"], own.0.id());
  assert_eq!(
    listed[0]["path"],
    inside.file_name().expect("name").to_str().expect("text")
  );
  assert!(listed[0]["start_time_unix_micros"].as_u64().expect("time") > 0);
  let result = executor.run(foreign.call());
  assert!(
    result.is_error() && result.text().contains("refused") && result.text().contains("outside"),
    "{}",
    result.text()
  );
  assert!(foreign.0.try_wait().expect("wait").is_none());
  executor.protect_executable(&inside).expect("protect configured CLI");
  let result = executor.run(own.call());
  assert!(
    result.is_error() && result.text().contains("protected"),
    "{}",
    result.text()
  );
  assert!(own.0.try_wait().expect("wait").is_none());
}

#[test]
fn run_processes_stops_only_inside_and_reports_already_gone() {
  let folder = folder();
  let program = copy(folder.path(), "app");
  let mut own = Sleeper::start(&program);
  let executor = RunProcesses::new(folder.path()).expect("scope");
  let result = executor.run(own.call());
  assert!(!result.is_error(), "{}", result.text());
  assert!(
    result.text().contains(if cfg!(windows) { "ended" } else { "closed" }),
    "{}",
    result.text()
  );
  own.0.wait().expect("reap");
  let result = executor.run(own.call());
  assert!(
    !result.is_error() && result.text().contains("already gone"),
    "{}",
    result.text()
  );
}

#[cfg(windows)]
#[test]
fn run_processes_requests_window_close_before_ending() {
  let folder = folder();
  let program = copy(folder.path(), "window-app");
  let mut own = Sleeper::configured(&program, true, None);
  let executor = RunProcesses::new(folder.path()).expect("scope");
  let result = executor.run(own.call());
  assert!(!result.is_error(), "{}", result.text());
  assert!(result.text().contains("closed"), "{}", result.text());
  assert_eq!(own.0.wait().expect("reap").code(), Some(0));
}

#[test]
fn run_processes_refuses_a_tree_with_an_outside_child_before_closing_parent() {
  let folder = folder();
  let worktree = folder.path().join("run");
  let outside = folder.path().join("other");
  fs::create_dir(&worktree).expect("root");
  fs::create_dir(&outside).expect("other");
  let parent_program = copy(&worktree, "parent");
  let child_program = copy(&outside, "child");
  let mut parent = Sleeper::configured(&parent_program, false, Some(&child_program));
  let child_pid: u32 = fs::read_to_string(parent_program.with_extension("ready"))
    .expect("child pid")
    .parse()
    .expect("pid");
  let executor = RunProcesses::new(&worktree).expect("scope");
  let refused = executor.run(parent.call());
  let parent_alive = parent.0.try_wait().expect("parent status").is_none();
  // This outside PID is a stand-in we just started, in our own scratch folder.
  let cleanup = RunProcesses::new(&outside)
    .expect("cleanup scope")
    .run(ProcessCall::Stop {
      pid: NonZeroU32::new(child_pid).expect("pid"),
    });
  assert!(!cleanup.is_error(), "{}", cleanup.text());
  assert!(
    refused.is_error() && refused.text().contains("outside"),
    "{}",
    refused.text()
  );
  assert!(parent_alive, "preflight refusal must send no close request");
}

#[test]
fn run_processes_refuses_the_executor_process_even_inside_its_scope() {
  let executable = std::env::current_exe().expect("test binary");
  let executor = RunProcesses::new(executable.parent().expect("folder")).expect("scope");
  let result = executor.run(ProcessCall::Stop {
    pid: NonZeroU32::new(std::process::id()).expect("own pid"),
  });
  assert!(
    result.is_error() && result.text().contains("protected"),
    "{}",
    result.text()
  );
}
