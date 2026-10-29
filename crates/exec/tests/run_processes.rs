//! Starts copies of this test binary as sleepers; all scratch and all processes belong to this test.

#[cfg(windows)]
#[path = "support/console_children.rs"]
mod console_children;
#[cfg(target_os = "macos")]
#[path = "../src/run_processes/macos/identity.rs"]
mod identity;
#[cfg(target_os = "macos")]
#[path = "support/process_term.rs"]
mod process_term;
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

use orchester_core::{orchester_tools::ProcessCall, time::Timestamp};
use orchester_exec::run_processes::RunProcesses;
use serde_json::Value;

/// Windows creation flags; macOS ignores them.
const DETACHED_PROCESS: u32 = 0x0000_0008;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

struct Sleeper(Child);

impl Sleeper {
  fn start(program: &Path) -> Self {
    Self::configured(program, false, None)
  }

  fn configured(program: &Path, window: bool, child_program: Option<&Path>) -> Self {
    Self::spawn(program, window, child_program, DETACHED_PROCESS)
  }

  /// Started like a console program the app starts: no window, but a console of its own (and so a conhost on
  /// Windows). Its child, if any, is started the same way.
  #[cfg(windows)]
  fn hidden_console(program: &Path, child_program: &Path) -> Self {
    Self::spawn(program, false, Some(child_program), CREATE_NO_WINDOW)
  }

  fn spawn(program: &Path, window: bool, child_program: Option<&Path>, flags: u32) -> Self {
    let ready = program.with_extension("ready");
    let mut command = Command::new(program);
    command.env("ORCHESTER_PROCESS_READY", &ready);
    if let Some(child) = child_program {
      command.env("ORCHESTER_PROCESS_CHILD", child);
    }
    if window {
      command.env("ORCHESTER_PROCESS_WINDOW", "1");
      if let Some(program) = child_program {
        command
          .env_remove("ORCHESTER_PROCESS_CHILD")
          .env("ORCHESTER_PROCESS_LATE_CHILD", program);
      }
    }
    command
      .args(["--exact", "run_processes_stand_in", "--ignored"])
      .env("ORCHESTER_PROCESS_STAND_IN", "1")
      .env("ORCHESTER_PROCESS_FLAGS", flags.to_string())
      .stdout(Stdio::null())
      .stderr(Stdio::null());
    #[cfg(windows)]
    {
      use std::os::windows::process::CommandExt;
      command.creation_flags(flags);
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
    process_window::wait_for_close(close_action).expect("window");
    return;
  }
  #[cfg(target_os = "macos")]
  if std::env::var_os("ORCHESTER_PROCESS_WINDOW").is_some() {
    process_term::wait_for_close(close_action).expect("term");
    return;
  }
  #[cfg(target_os = "macos")]
  if let Some(program) = std::env::var_os("ORCHESTER_PROCESS_EXEC") {
    use std::os::unix::process::CommandExt;
    let ready = std::env::var_os("ORCHESTER_PROCESS_READY").expect("ready path");
    fs::write(&ready, "ready").expect("ready");
    wait_for_file(&PathBuf::from(&program).with_extension("go"));
    panic!(
      "exec failed: {}",
      Command::new(program)
        .args(["--exact", "run_processes_stand_in", "--ignored"])
        .env_remove("ORCHESTER_PROCESS_EXEC")
        .env(
          "ORCHESTER_PROCESS_READY",
          PathBuf::from(ready).with_extension("after-exec")
        )
        .exec()
    );
  }
  let child = std::env::var_os("ORCHESTER_PROCESS_CHILD").map(|program| spawn_child(program, true));
  fs::write(
    std::env::var_os("ORCHESTER_PROCESS_READY").expect("ready path"),
    child
      .as_ref()
      .map_or_else(|| "ready".to_owned(), |child| child.id().to_string()),
  )
  .expect("ready");
  std::thread::sleep(Duration::from_secs(60));
}

fn close_action() -> std::io::Result<()> {
  if let Some(program) = std::env::var_os("ORCHESTER_PROCESS_LATE_CHILD") {
    let child = spawn_child(program, false);
    fs::write(
      PathBuf::from(std::env::var_os("ORCHESTER_PROCESS_READY").expect("ready")).with_extension("late-pid"),
      child.id().to_string(),
    )?;
    // Exit immediately after spawning: the executor must discover the child via an exited parent.
    std::process::exit(0);
  }
  Ok(())
}

fn spawn_child(program: std::ffi::OsString, wait: bool) -> Child {
  let child_ready = PathBuf::from(&program).with_extension("child-ready");
  let mut command = Command::new(program);
  command
    .args(["--exact", "run_processes_stand_in", "--ignored"])
    .env("ORCHESTER_PROCESS_READY", &child_ready)
    .env_remove("ORCHESTER_PROCESS_CHILD")
    .env_remove("ORCHESTER_PROCESS_LATE_CHILD")
    .env_remove("ORCHESTER_PROCESS_WINDOW")
    .stdout(Stdio::null())
    .stderr(Stdio::null());
  #[cfg(windows)]
  {
    use std::os::windows::process::CommandExt;
    let flags = std::env::var("ORCHESTER_PROCESS_FLAGS")
      .ok()
      .and_then(|flags| flags.parse().ok())
      .unwrap_or(DETACHED_PROCESS);
    command.creation_flags(flags);
  }
  let child = command.spawn().expect("child");
  if wait {
    wait_for_file(&child_ready);
  }
  child
}

fn wait_for_file(path: &Path) {
  let deadline = std::time::Instant::now() + Duration::from_secs(10);
  while !path.exists() {
    assert!(
      std::time::Instant::now() < deadline,
      "stand-in did not become ready: {}",
      path.display()
    );
    std::thread::sleep(Duration::from_millis(10));
  }
}

fn folder() -> tempfile::TempDir {
  let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/run-processes-exec");
  fs::create_dir_all(&base).expect("scratch");
  tempfile::tempdir_in(base).expect("folder")
}

/// The executor of a run created a minute ago, so every stand-in a test starts is the run's.
fn processes_of(worktree: &Path) -> RunProcesses {
  let run_created = Timestamp::from_unix_millis(Timestamp::now().unix_millis() - 60_000);
  RunProcesses::new(worktree, run_created).expect("scope")
}

/// Polls `check` for up to ten seconds; the last answer is returned either way.
#[cfg(windows)]
fn eventually<T>(mut check: impl FnMut() -> (bool, T)) -> T {
  let deadline = std::time::Instant::now() + Duration::from_secs(10);
  loop {
    let (done, value) = check();
    if done || std::time::Instant::now() >= deadline {
      return value;
    }
    std::thread::sleep(Duration::from_millis(20));
  }
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
  let executor = processes_of(&worktree);
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
  assert!(!result.text().contains("other-run"), "no path: {}", result.text());
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
  let executor = processes_of(folder.path());
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

#[cfg(any(windows, target_os = "macos"))]
#[test]
fn run_processes_requests_close_before_ending() {
  let folder = folder();
  let program = copy(folder.path(), "window-app");
  let mut own = Sleeper::configured(&program, true, None);
  let executor = processes_of(folder.path());
  let result = executor.run(own.call());
  assert!(!result.is_error(), "{}", result.text());
  assert!(result.text().contains("closed"), "{}", result.text());
  assert_eq!(own.0.wait().expect("reap").code(), Some(0));
}

#[test]
fn run_processes_stops_the_parent_and_leaves_an_outside_child_running() {
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
  let result = processes_of(&worktree).run(parent.call());
  // This outside PID is a stand-in we just started, in our own scratch folder.
  let outside_scope = processes_of(&outside);
  let listed = outside_scope.run(ProcessCall::List);
  let cleanup = outside_scope.run(ProcessCall::Stop {
    pid: NonZeroU32::new(child_pid).expect("pid"),
  });
  assert!(!cleanup.is_error(), "{}", cleanup.text());
  assert!(!result.is_error(), "{}", result.text());
  assert!(
    parent.0.try_wait().expect("parent status").is_some(),
    "{}",
    result.text()
  );
  let line = format!("pid {child_pid}: not stopped, its executable is outside this run's worktree; still running");
  assert!(result.text().contains(&line), "{}", result.text());
  assert!(!result.text().contains("other"), "no outside path: {}", result.text());
  assert!(
    listed.text().contains(&format!("\"pid\":{child_pid}")),
    "{}",
    listed.text()
  );
}

/// Every console program has a conhost child outside the worktree; it never blocks the stop and is not touched.
#[cfg(windows)]
#[test]
fn run_processes_stops_a_console_tree_and_leaves_its_conhost() {
  let folder = folder();
  let parent_program = copy(folder.path(), "console-parent");
  let child_program = copy(folder.path(), "console-child");
  let mut parent = Sleeper::hidden_console(&parent_program, &child_program);
  let child_pid: u32 = fs::read_to_string(parent_program.with_extension("ready"))
    .expect("child pid")
    .parse()
    .expect("pid");
  let conhosts = eventually(|| {
    let found: Vec<u32> = [parent.0.id(), child_pid]
      .into_iter()
      .flat_map(|pid| console_children::children_named(pid, "conhost.exe").expect("snapshot"))
      .collect();
    (found.len() == 2, found)
  });
  assert_eq!(conhosts.len(), 2, "each console stand-in has its console host");
  let executor = processes_of(folder.path());
  let result = executor.run(parent.call());
  assert!(!result.is_error(), "{}", result.text());
  assert!(
    result.text().contains(&format!("pid {child_pid}: ended")),
    "{}",
    result.text()
  );
  for conhost in conhosts {
    let prefix = format!("pid {conhost}: not stopped, its executable is outside this run's worktree");
    assert!(
      result
        .text()
        .lines()
        .any(|line| line.starts_with(&prefix) && line.ends_with("(conhost.exe)")),
      "{}",
      result.text()
    );
  }
  assert!(
    !result.text().contains("System32"),
    "no outside path: {}",
    result.text()
  );
  let exited = eventually(|| {
    let exited = parent.0.try_wait().expect("parent status").is_some();
    (exited, exited)
  });
  assert!(exited, "{}", result.text());
  let remaining = eventually(|| {
    let listed = executor.run(ProcessCall::List);
    (listed.text() == "[]", listed.text().to_owned())
  });
  assert_eq!(remaining, "[]");
}

#[test]
fn run_processes_refuses_a_process_older_than_the_run() {
  let folder = folder();
  let program = copy(folder.path(), "older");
  let mut older = Sleeper::start(&program);
  std::thread::sleep(Duration::from_millis(20));
  let executor = RunProcesses::new(folder.path(), Timestamp::now()).expect("scope");
  let listed = executor.run(ProcessCall::List);
  assert_eq!(listed.text(), "[]");
  let refused = executor.run(older.call());
  assert!(
    refused.is_error() && refused.text().contains("refused") && refused.text().contains("started before this run"),
    "{}",
    refused.text()
  );
  assert!(!refused.text().contains("older"), "no path: {}", refused.text());
  assert!(older.0.try_wait().expect("status").is_none());
}

#[test]
fn run_processes_refuses_the_executor_process_even_inside_its_scope() {
  let executable = std::env::current_exe().expect("test binary");
  let executor = processes_of(executable.parent().expect("folder"));
  let result = executor.run(ProcessCall::Stop {
    pid: NonZeroU32::new(std::process::id()).expect("own pid"),
  });
  assert!(
    result.is_error() && result.text().contains("protected"),
    "{}",
    result.text()
  );
}
#[test]
fn run_processes_stops_a_child_spawned_during_close_after_parent_exits() {
  let folder = folder();
  let parent_program = copy(folder.path(), "closing-parent");
  let child_program = copy(folder.path(), "late-child");
  let mut parent = Sleeper::configured(&parent_program, true, Some(&child_program));
  let executor = processes_of(folder.path());
  let result = executor.run(parent.call());
  let pid_file = parent_program.with_extension("late-pid");
  wait_for_file(&pid_file);
  let child_pid: u32 = fs::read_to_string(pid_file).expect("late pid").parse().expect("pid");
  let remaining = executor.run(ProcessCall::List);
  // Clean up our own child even if the regression leaves it alive, before asserting.
  let cleanup = executor.run(ProcessCall::Stop {
    pid: NonZeroU32::new(child_pid).expect("pid"),
  });
  assert!(!cleanup.is_error(), "{}", cleanup.text());
  assert!(!result.is_error(), "{}", result.text());
  assert!(
    result.text().contains(&format!("pid {child_pid}:")),
    "{}",
    result.text()
  );
  assert!(!remaining.is_error(), "{}", remaining.text());
  assert_eq!(
    serde_json::from_str::<Vec<Value>>(remaining.text())
      .expect("list")
      .len(),
    0
  );
  assert_eq!(parent.0.wait().expect("parent").code(), Some(0));
}

#[test]
fn run_processes_listing_tolerates_processes_exiting_during_inspection() {
  let folder = folder();
  let program = copy(folder.path(), "short-lived");
  let executor = processes_of(folder.path());
  for _ in 0..32 {
    let mut child = Command::new(&program)
      .args(["--list"])
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .spawn()
      .expect("short-lived child");
    let listed = executor.run(ProcessCall::List);
    child.wait().expect("reap");
    assert!(!listed.is_error(), "{}", listed.text());
  }
}

#[cfg(target_os = "macos")]
#[test]
fn run_processes_audit_token_refuses_exec_after_validation() {
  let folder = folder();
  let worktree = folder.path().join("run");
  let other = folder.path().join("other");
  fs::create_dir(&worktree).expect("run");
  fs::create_dir(&other).expect("other");
  let inside = copy(&worktree, "before-exec");
  let outside = copy(&other, "after-exec");
  let ready = inside.with_extension("ready");
  let child = Command::new(&inside)
    .args(["--exact", "run_processes_stand_in", "--ignored"])
    .env("ORCHESTER_PROCESS_STAND_IN", "1")
    .env("ORCHESTER_PROCESS_READY", &ready)
    .env("ORCHESTER_PROCESS_EXEC", &outside)
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .expect("exec stand-in");
  let mut child = Sleeper(child);
  wait_for_file(&ready);
  let before = identity::read(child.0.id()).expect("identity").expect("live");
  assert!(!before.exited());
  let token = before.token();
  assert_eq!(
    fs::canonicalize(token.image().expect("image").expect("path")).expect("resolved"),
    fs::canonicalize(&inside).expect("inside")
  );
  fs::write(outside.with_extension("go"), "exec").expect("go");
  wait_for_file(&ready.with_extension("after-exec"));
  let after = identity::read(child.0.id()).expect("after").expect("live");
  assert_eq!(before.unique.unique_id, after.unique.unique_id);
  assert!(before.token() != after.token(), "exec must advance pidversion");
  assert!(token.image().expect("stale image").is_none());
  for signal in [libc::SIGTERM, libc::SIGKILL] {
    assert_eq!(
      token.signal(signal).expect_err("stale token").raw_os_error(),
      Some(libc::ESRCH)
    );
    assert!(child.0.try_wait().expect("outside status").is_none());
  }
  let refused = processes_of(&worktree).run(child.call());
  assert!(
    refused.is_error() && refused.text().contains("outside"),
    "{}",
    refused.text()
  );
}

#[cfg(target_os = "macos")]
#[test]
fn run_processes_audit_lookup_treats_exit_between_queries_as_gone() {
  let folder = folder();
  let program = copy(folder.path(), "exiting");
  let mut child = Sleeper::start(&program);
  let before = identity::read(child.0.id()).expect("BSD identity").expect("live");
  let token = before.token();
  // Reproduce exit after identity lookup but before executable lookup, using only our own stand-in.
  child.0.kill().expect("end owned stand-in");
  child.0.wait().expect("reap");
  assert!(token.image().expect("exit is disappearance").is_none());
  assert!(identity::read(child.0.id()).expect("exited identity").is_none());
  let result = processes_of(folder.path()).run(child.call());
  assert!(
    !result.is_error() && result.text().contains("already gone"),
    "{}",
    result.text()
  );
}

/// A child that exited is still described by its start time until it is reaped, as when it was alive.
#[cfg(target_os = "macos")]
#[test]
fn run_processes_start_time_of_an_exited_child_waiting_to_be_reaped() {
  let folder = folder();
  let program = copy(folder.path(), "zombie");
  let mut child = Sleeper::start(&program);
  let pid = child.0.id();
  let alive = identity::read(pid).expect("BSD identity").expect("live");
  let micros = alive.bsd.pbi_start_tvsec * 1_000_000 + alive.bsd.pbi_start_tvusec;
  assert_eq!(identity::started_even_exited(pid).expect("alive"), Some(micros));
  child.0.kill().expect("end owned stand-in");
  while identity::read(pid).expect("identity").is_some() {
    std::thread::sleep(Duration::from_millis(5));
  }
  assert_eq!(identity::started_even_exited(pid).expect("exited"), Some(micros));
  child.0.wait().expect("reap");
  assert_eq!(identity::started_even_exited(pid).expect("reaped"), None);
}
