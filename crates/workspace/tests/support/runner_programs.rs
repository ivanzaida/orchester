//! The real `orchester-runner` and the fake agent CLIs, for tests outside the runner crate that start them.
//!
//! Each test process builds them once, always with the same command (`cargo build -p orchester-runner --features
//! test-agent --bins`), and starts copies in a folder of its own, `<target>/debug/test-programs/<test>-<pid>/`,
//! never the files Cargo built: another test binary or a `cargo build` running at the same time may rebuild those,
//! with other features, while a test starts them. Cargo puts them where every build of the runner puts its own (on
//! Windows even the file in `deps/` has no hash), so a build that starts once this one ended could swap them while
//! they are copied. Each program's size and modification time are noted as soon as Cargo reports it built, while
//! Cargo still holds its build folder or has only just let it go, and the copies are kept only if both are the same
//! once copied; otherwise the build and the copies start over.
//!
//! The runner looks up nothing beside its own program (its relay for Claude Code's permission prompt is that
//! program itself), and the fakes reach the runner by the absolute paths their connections record, so the copies
//! run as the originals would.
//!
//! orchester-workspace's and orchester-app's tests share this file (`#[path]`), so they cannot drift apart into
//! differently built runners.

#![allow(dead_code, reason = "each test binary starts some of the programs")]

use std::{
  env,
  ffi::OsStr,
  fs::{self, File, OpenOptions},
  io::{self, BufRead, BufReader},
  path::{Path, PathBuf},
  process::{self, Command, Stdio},
  sync::OnceLock,
  time::SystemTime,
};

use serde_json::Value;

/// The programs the build makes that the tests start, in the order of [`Programs`]' fields.
const NAMES: [&str; 3] = ["orchester-runner", "orchester-fake-codex", "orchester-fake-claude"];
/// How many times programs swapped while they were copied may make the build and the copies start over.
const ATTEMPTS: usize = 5;
/// The folder beside the built programs that holds every test process's copies.
const FOLDER: &str = "test-programs";
/// Locked by the test process that sweeps the folder and claims a copy folder in it, one at a time.
const SWEEP_LOCK: &str = "sweep.lock";
/// Locked inside a copy folder for as long as its test process runs.
const IN_USE_LOCK: &str = "in-use.lock";
/// Variables Cargo sets for the test it runs. The build must not inherit them: build scripts that rerun when one
/// changes (ring's, for `CARGO_MANIFEST_DIR`) would rebuild for every switch between it and `cargo test`.
const TEST_VARIABLES: [&str; 7] = [
  "CARGO_MANIFEST_DIR",
  "CARGO_MANIFEST_PATH",
  "CARGO_CRATE_NAME",
  "CARGO_PRIMARY_PACKAGE",
  "CARGO_TARGET_TMPDIR",
  "CARGO_RUSTC_CURRENT_DIR",
  "OUT_DIR",
];
const TEST_VARIABLE_PREFIXES: [&str; 2] = ["CARGO_PKG_", "CARGO_BIN_"];

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
    let mut built = cargo_build();
    let beside = built[0].path.parent().expect("Cargo's programs folder");
    let (folder, in_use) = claim(&beside.join(FOLDER));
    for _ in 0..ATTEMPTS {
      let copies = copy_into(&built, &folder);
      if built.iter().all(Built::unchanged) {
        let [runner, fake_codex, fake_claude] = copies;
        return Self {
          runner,
          fake_codex,
          fake_claude,
          _in_use: in_use,
        };
      }
      built = cargo_build();
    }
    panic!("another build swapped the runner while it was copied, {ATTEMPTS} times");
  }
}

/// A program Cargo built, with its size and modification time when Cargo reported it.
struct Built {
  path: PathBuf,
  stamp: (u64, SystemTime),
}

impl Built {
  fn noted(path: PathBuf) -> Self {
    let stamp = stamp(&path);
    Self { path, stamp }
  }

  /// Whether the file is still the one Cargo reported: another build writes a new file or links another one there.
  fn unchanged(&self) -> bool {
    stamp(&self.path) == self.stamp
  }
}

fn stamp(path: &Path) -> (u64, SystemTime) {
  let metadata = fs::metadata(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
  (
    metadata.len(),
    metadata.modified().expect("the file's modification time"),
  )
}

/// Builds the programs and notes them as Cargo reports each.
fn cargo_build() -> [Built; 3] {
  let mut cargo = Command::new(env!("CARGO"));
  cargo
    .args([
      "build",
      "--quiet",
      "--message-format=json-render-diagnostics",
      "-p",
      "orchester-runner",
      "--features",
      "test-agent",
      "--bins",
    ])
    // The crate's folder when the test was built: a test program started directly may run anywhere.
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .stdout(Stdio::piped());
  for (name, _) in env::vars_os() {
    if is_test_variable(&name) {
      cargo.env_remove(name);
    }
  }
  // A test program started directly, not by `cargo test`, has no RUSTUP_TOOLCHAIN: rustup would then compile the
  // dependencies, whose folders lie outside this repository's rust-toolchain.toml, with its default toolchain.
  if let Some(toolchain) = option_env!("RUSTUP_TOOLCHAIN") {
    cargo.env("RUSTUP_TOOLCHAIN", toolchain);
  }
  let mut child = cargo.spawn().expect("run cargo");
  let messages = BufReader::new(child.stdout.take().expect("cargo's piped output"));
  let mut built: [Option<Built>; 3] = Default::default();
  for line in messages.lines() {
    let message: Value = serde_json::from_str(&line.expect("cargo's output")).expect("a cargo message");
    if let Some((index, path)) = program_built(&message) {
      built[index] = Some(Built::noted(path));
    }
  }
  let status = child.wait().expect("wait for cargo");
  assert!(
    status.success(),
    "cargo build -p orchester-runner --features test-agent --bins: {status}"
  );
  built.map(|program| program.expect("cargo built every program"))
}

fn is_test_variable(name: &OsStr) -> bool {
  name.to_str().is_some_and(|name| {
    TEST_VARIABLES.contains(&name) || TEST_VARIABLE_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
  })
}

/// The index in [`NAMES`] and the file of a program the message says Cargo built.
fn program_built(message: &Value) -> Option<(usize, PathBuf)> {
  if message["reason"] != "compiler-artifact" {
    return None;
  }
  let index = NAMES.iter().position(|name| message["target"]["name"] == *name)?;
  Some((index, PathBuf::from(message["executable"].as_str()?)))
}

fn copy_into(built: &[Built; 3], folder: &Path) -> [PathBuf; 3] {
  built.each_ref().map(|Built { path, .. }| {
    let copy = folder.join(path.file_name().expect("a program's file name"));
    fs::copy(path, &copy).unwrap_or_else(|error| panic!("copy {}: {error}", path.display()));
    copy
  })
}

/// Removes the copy folders of test processes that have ended, then makes a new folder in `root` this process's,
/// locked until it ends. One process at a time does both, so none sees another's folder before it is locked.
fn claim(root: &Path) -> (PathBuf, File) {
  fs::create_dir_all(root).expect("the test programs' folder");
  let sweep = lock_file(&root.join(SWEEP_LOCK)).expect("the test programs' sweep lock");
  sweep.lock().expect("lock the test programs' folder");
  remove_ended(root);
  let folder = new_folder(root);
  let in_use = lock_file(&folder.join(IN_USE_LOCK)).expect("this test process's lock");
  in_use.try_lock().expect("a new folder's lock is free");
  (folder, in_use)
}

/// `<test>-<pid>` in `root`, or with a further number when an ended process with the same id left it behind: a
/// program of that process may still run from it.
fn new_folder(root: &Path) -> PathBuf {
  let test = env::current_exe().expect("the test's path");
  let name = format!(
    "{}-{}",
    test.file_stem().expect("the test's file name").to_string_lossy(),
    process::id()
  );
  let mut folder = root.join(&name);
  let mut number = 0;
  loop {
    match fs::create_dir(&folder) {
      Ok(()) => return folder,
      Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
        number += 1;
        folder = root.join(format!("{name}-{number}"));
      }
      Err(error) => panic!("create {}: {error}", folder.display()),
    }
  }
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
