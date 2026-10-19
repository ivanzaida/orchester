//! The real `orchester-runner` and the fake agent CLIs, for tests outside the runner crate that start them.
//!
//! Each test process builds them once, always with the same command (`cargo build -p orchester-runner --features
//! test-agent --bins`), and starts copies in a folder of its own, `<target>/<profile>/test-programs/<test>-<pid>/`,
//! never the shared files in `<target>/<profile>/`: another test binary or a `cargo build` running at the same time
//! may rebuild those, with other features, while a test starts them. The runner looks up nothing beside its own
//! program (its relay for Claude Code's permission prompt is that program itself), and the fakes reach the runner
//! by the absolute paths their connections record, so the copies run as the originals would.
//!
//! orchester-workspace's and orchester-app's tests share this file (`#[path]`), so they cannot drift apart into
//! differently built runners.

#![allow(dead_code, reason = "each test binary starts some of the programs")]

use std::{
  env,
  fs::{self, File, OpenOptions},
  io,
  path::{Path, PathBuf},
  process::{self, Command},
  sync::OnceLock,
};

/// The folder under `<target>/<profile>/` that holds every test process's copies.
const FOLDER: &str = "test-programs";
/// Locked by the test process that sweeps the folder and claims a copy folder in it, one at a time.
const SWEEP_LOCK: &str = "sweep.lock";
/// Locked inside a copy folder for as long as its test process runs.
const IN_USE_LOCK: &str = "in-use.lock";

/// This test process's copies of the programs.
pub struct Programs {
  pub runner: PathBuf,
  pub fake_codex: PathBuf,
  pub fake_claude: PathBuf,
  /// Locked while the process runs, so no other test process removes the copies; the OS releases it at exit.
  _in_use: File,
}

/// The programs, built and copied by the first test of the process that asks.
pub fn programs() -> &'static Programs {
  static COPIED: OnceLock<Programs> = OnceLock::new();
  COPIED.get_or_init(Programs::build)
}

impl Programs {
  fn build() -> Self {
    cargo_build();
    let test = env::current_exe().expect("the test's path");
    let profile = test
      .parent()
      .and_then(Path::parent)
      .expect("tests live in <target>/<profile>/deps");
    let name = test.file_stem().expect("the test's file name").to_string_lossy();
    let (folder, in_use) = claim(&profile.join(FOLDER), &format!("{name}-{}", process::id()));
    let copy = |program: &str| {
      let file = format!("{program}{}", env::consts::EXE_SUFFIX);
      let copy = folder.join(&file);
      fs::copy(profile.join(&file), &copy).unwrap_or_else(|error| panic!("copy {file}: {error}"));
      copy
    };
    Self {
      runner: copy("orchester-runner"),
      fake_codex: copy("orchester-fake-codex"),
      fake_claude: copy("orchester-fake-claude"),
      _in_use: in_use,
    }
  }
}

fn cargo_build() {
  let mut cargo = Command::new(env!("CARGO"));
  cargo.args([
    "build",
    "--quiet",
    "-p",
    "orchester-runner",
    "--features",
    "test-agent",
    "--bins",
  ]);
  // A test program started directly, not by `cargo test`, has no RUSTUP_TOOLCHAIN: rustup would then compile the
  // dependencies, whose folders lie outside this repository's rust-toolchain.toml, with its default toolchain.
  if let Some(toolchain) = option_env!("RUSTUP_TOOLCHAIN") {
    cargo.env("RUSTUP_TOOLCHAIN", toolchain);
  }
  let status = cargo.status().expect("run cargo");
  assert!(
    status.success(),
    "cargo build -p orchester-runner --features test-agent --bins: {status}"
  );
}

/// Removes the copy folders of test processes that have ended, then makes `<root>/<name>` this process's, locked
/// until it ends. One process at a time does both, so none sees another's folder before it is locked.
fn claim(root: &Path, name: &str) -> (PathBuf, File) {
  fs::create_dir_all(root).expect("the test programs' folder");
  let sweep = lock_file(&root.join(SWEEP_LOCK)).expect("the test programs' sweep lock");
  sweep.lock().expect("lock the test programs' folder");
  remove_ended(root);
  let folder = root.join(name);
  fs::create_dir_all(&folder).expect("this test process's programs folder");
  let in_use = lock_file(&folder.join(IN_USE_LOCK)).expect("this test process's lock");
  in_use.try_lock().expect("a folder no running test process holds");
  (folder, in_use)
}

/// Removes every copy folder whose lock no running test process holds.
fn remove_ended(root: &Path) {
  for entry in fs::read_dir(root).expect("read the test programs' folder") {
    let folder = entry.expect("a test programs' folder entry").path();
    if !folder.is_dir() || held(&folder.join(IN_USE_LOCK)) {
      continue;
    }
    // Fails on Windows while a runner or fake that an ended test left behind still runs from the folder. Nothing
    // is lost: the next test process that starts once that program has ended removes the rest.
    let _ = fs::remove_dir_all(&folder);
  }
}

/// Whether a running test process holds the lock at `path`. A folder without one was left by a process that ended
/// before it locked it, or was partly removed already.
fn held(path: &Path) -> bool {
  match OpenOptions::new().read(true).write(true).open(path) {
    Ok(file) => file.try_lock().is_err(),
    Err(error) => error.kind() != io::ErrorKind::NotFound,
  }
}

fn lock_file(path: &Path) -> io::Result<File> {
  OpenOptions::new()
    .read(true)
    .write(true)
    .create(true)
    .truncate(false)
    .open(path)
}
