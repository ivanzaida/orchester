//! A parallel step's worktree starts with the run's build output (ORC-544): seeding it as hard links, a real cargo
//! build in the step that reuses it without changing a file of the run's `target/`, and removing the step's worktree.

use std::{
  collections::BTreeMap,
  fs,
  path::{Path, PathBuf},
  process::{Command, Output},
  time::SystemTime,
};

use orchester_core::id::{RunId, StepExecutionId};
use orchester_runner::worktree::{Worktrees, warm_start};
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) -> String {
  let output = Command::new("git")
    .arg("-C")
    .arg(dir)
    .args(args)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?} failed: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn write(path: &Path, contents: &str) {
  fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
  fs::write(path, contents).expect("write the file");
}

/// The files of a profile folder: linked, copied and left out of a seeding.
const LINKED: [&str; 3] = [
  "deps/libwarm-1.rlib",
  "deps/libwarm-1.rmeta",
  "incremental/warm-4/s-5/dep-graph.bin",
];
const COPIED: [&str; 9] = [
  ".fingerprint/warm-1/lib-warm",
  "deps/warm-1.d",
  "deps/derive-1.dll",
  "deps/derive-1.pdb",
  "deps/libderive-1.so",
  "build/warm-2/build-script-build",
  "build/warm-3/out/generated.rs",
  "build/warm-3/output",
  "incremental/warm-4/s-5.lock",
];
const LEFT_OUT: [&str; 6] = [
  ".cargo-lock",
  "deps/warm-1.exe",
  "deps/warm-1.pdb",
  "deps/warm-2",
  "deps/warm-2.dSYM/Contents/Info.plist",
  "warm.exe",
];

/// A `target/` as cargo leaves it, with a profile folder, one of a target triple and a scratch folder tests write to.
fn built_target(worktree: &Path) {
  let target = worktree.join("target");
  for profile in [
    target.join("debug"),
    target.join("x86_64-unknown-linux-gnu").join("release"),
  ] {
    for file in LINKED.iter().chain(&COPIED).chain(&LEFT_OUT) {
      write(&profile.join(file), file);
    }
  }
  write(&target.join("tmp/scratch.txt"), "scratch");
  write(&target.join("doc/index.html"), "docs");
}

/// Whether `relative` in `step`'s `target/` is the same file as in `run`'s: a write through the step's name shows
/// through the run's. Puts the run's contents back.
fn shared(run: &Path, step: &Path, relative: &str) -> bool {
  let original = fs::read(run.join("target").join(relative)).expect("the run's file");
  fs::write(step.join("target").join(relative), b"changed by the step").expect("write the step's file");
  let shared = fs::read(run.join("target").join(relative)).expect("the run's file") != original;
  fs::write(run.join("target").join(relative), &original).expect("put the run's file back");
  shared
}

#[test]
fn seeding_links_the_artifacts_and_copies_what_a_build_writes_in_place() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let (run, step) = (folder.path().join("run"), folder.path().join("step"));
  built_target(&run);
  fs::create_dir(&step).expect("the step's worktree");

  let seeded = warm_start::seed(&run, &step).expect("seeded");

  assert_eq!((seeded.linked, seeded.copied), (6, 18), "{seeded:?}");
  for profile in ["debug", "x86_64-unknown-linux-gnu/release"] {
    for linked in LINKED {
      assert!(
        shared(&run, &step, &format!("{profile}/{linked}")),
        "{profile}/{linked} is linked"
      );
    }
    for copied in COPIED {
      let relative = format!("{profile}/{copied}");
      let times = [&run, &step].map(|worktree| modified(&worktree.join("target").join(&relative)));
      assert_eq!(times[0], times[1], "{relative} keeps its time");
      assert!(!shared(&run, &step, &relative), "{relative} is copied");
    }
    for left_out in LEFT_OUT {
      assert!(
        !step.join("target").join(profile).join(left_out).exists(),
        "{profile}/{left_out} is left out"
      );
    }
  }
  assert!(!step.join("target/tmp").exists(), "scratch folders are not seeded");
  assert!(!step.join("target/doc").exists());
}

#[test]
fn without_a_run_target_the_step_starts_as_before() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let (run, step) = (folder.path().join("run"), folder.path().join("step"));
  fs::create_dir_all(run.join("target/tmp")).expect("a target without a profile folder");
  fs::create_dir(&step).expect("the step's worktree");

  assert_eq!(warm_start::seed(&folder.path().join("none"), &step), None);
  assert_eq!(warm_start::seed(&run, &step), None);
  assert!(!step.join("target").exists());
}

/// The system's temporary folder and the build's own (`target/tmp`) lie on different volumes on the machines that
/// build Orchester today (`C:` and the repository's drive); where they do not, the seeding must go through.
#[test]
fn across_volumes_the_step_starts_as_before() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let other = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("a folder in the build's own temporary folder");
  let run = folder.path().join("run");
  built_target(&run);
  let step = other.path().join("step");
  fs::create_dir(&step).expect("the step's worktree");
  let probe = run.join("target/debug/deps/libwarm-1.rlib");
  let one_volume = fs::hard_link(&probe, other.path().join("probe")).is_ok();

  assert_eq!(warm_start::seed(&run, &step).is_some(), one_volume);
  assert_eq!(
    step.join("target").exists(),
    one_volume,
    "a seeding that cannot link leaves nothing"
  );
}

/// A repository holding a package with a build script that writes its `OUT_DIR`, `target/` ignored.
fn package() -> TempDir {
  let repo = tempfile::tempdir().expect("temporary directory");
  let root = repo.path();
  git(root, &["init", "--quiet", "--initial-branch=main"]);
  git(root, &["config", "user.name", "Orchester Test"]);
  git(root, &["config", "user.email", "test@orchester.invalid"]);
  git(root, &["config", "core.autocrlf", "false"]);
  git(root, &["config", "commit.gpgsign", "false"]);
  write(
    &root.join("Cargo.toml"),
    "[package]\nname = \"warm\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
  );
  // The build script watches a folder, as the app's watches its locales: the folder's own time counts.
  write(
    &root.join("build.rs"),
    "fn main() {\n  let text = std::fs::read_to_string(\"assets/generated.txt\").expect(\"generated.txt\");\n  let out \
     = std::path::PathBuf::from(std::env::var(\"OUT_DIR\").expect(\"OUT_DIR\"));\n  \
     std::fs::write(out.join(\"generated.rs\"), format!(\"pub const GENERATED: &str = {:?};\", \
     text.trim())).expect(\"write\");\n  println!(\"cargo:rerun-if-changed=assets\");\n}\n",
  );
  write(&root.join("assets/generated.txt"), "first\n");
  write(
    &root.join("src/lib.rs"),
    "include!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));\n\npub fn greeting() -> &'static str {\n  \"hello\"\n}\n",
  );
  write(
    &root.join("src/main.rs"),
    "fn main() {\n  println!(\"{} {}\", warm::greeting(), warm::GENERATED);\n}\n",
  );
  write(&root.join(".gitignore"), "target/\nCargo.lock\n");
  git(root, &["add", "."]);
  git(root, &["commit", "--quiet", "-m", "package"]);
  fs::create_dir(root.join(".orchester")).expect("create .orchester");
  fs::write(root.join(".git/info/exclude"), ".orchester/\n").expect("exclude .orchester");
  repo
}

/// `cargo <command> -v` (`build`, `clippy`) of every target in `worktree`, which must succeed; its standard error.
fn cargo_ok(worktree: &Path, command: &str) -> String {
  let output = cargo(worktree, &[command, "-v", "--offline", "--all-targets"]);
  let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
  assert!(output.status.success(), "cargo {command} failed: {stderr}");
  stderr
}

fn cargo(worktree: &Path, args: &[&str]) -> Output {
  Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
    .args(args)
    .current_dir(worktree)
    .env_remove("CARGO_TARGET_DIR")
    .env_remove("CARGO_BUILD_TARGET_DIR")
    .output()
    .expect("run cargo")
}

fn greets(worktree: &Path) -> String {
  let program = worktree
    .join("target/debug")
    .join(format!("warm{}", std::env::consts::EXE_SUFFIX));
  let output = Command::new(program).output().expect("run the program");
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn modified(path: &Path) -> SystemTime {
  fs::metadata(path)
    .and_then(|metadata| metadata.modified())
    .expect("its time")
}

/// Every file under `folder` with its contents and time.
fn snapshot(folder: &Path) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime)> {
  let mut files = BTreeMap::new();
  let mut pending = vec![folder.to_path_buf()];
  while let Some(current) = pending.pop() {
    for entry in fs::read_dir(&current).expect("read the folder") {
      let path = entry.expect("an entry").path();
      if path.is_dir() {
        pending.push(path);
      } else {
        let contents = fs::read(&path).expect("read the file");
        files.insert(
          path.strip_prefix(folder).expect("inside").to_path_buf(),
          (contents, modified(&path)),
        );
      }
    }
  }
  files
}

/// A run's worktree built by cargo, and a step's worktree beside it, seeded from it.
struct Fixture {
  repo: TempDir,
  run: PathBuf,
  step: PathBuf,
  execution: StepExecutionId,
}

impl Fixture {
  fn seeded() -> Self {
    let repo = package();
    let worktrees = Worktrees::new(repo.path()).expect("worktrees");
    let run = worktrees
      .create(RunId::generate(), "orchester/run-1", "main")
      .expect("the run's worktree")
      .path()
      .to_path_buf();
    cargo_ok(&run, "build");
    cargo_ok(&run, "clippy");
    assert_eq!(greets(&run), "hello first");
    let execution = StepExecutionId::generate();
    let step = worktrees
      .create_step(execution, "orchester/run-1--check-1", "orchester/run-1")
      .expect("the step's worktree")
      .path()
      .to_path_buf();
    let seeded = warm_start::seed(&run, &step).expect("seeded");
    // Six files and the folders `assets` and `src`.
    assert!(seeded.linked > 0 && seeded.aligned == 8, "{seeded:?}");
    Self {
      repo,
      run,
      step,
      execution,
    }
  }
}

#[test]
fn a_build_in_the_seeded_step_reuses_the_run_build_and_never_changes_its_files() {
  let fixture = Fixture::seeded();
  let before = snapshot(&fixture.run.join("target"));

  let checked = cargo_ok(&fixture.step, "clippy");
  assert!(
    checked.contains("Fresh warm"),
    "the first clippy reuses the run's: {checked}"
  );
  assert!(!checked.contains("Checking warm"), "{checked}");
  // The programs were left out, so cargo builds them again; the library and the build script stay as the run built
  // them.
  let built = cargo_ok(&fixture.step, "build");
  assert!(built.contains("couldn't read metadata for file"), "{built}");
  assert!(
    !built.contains("--crate-type lib") && !built.contains("build_script_build"),
    "{built}"
  );
  assert!(
    !built.contains("build-script-build`"),
    "the build script does not run again: {built}"
  );
  assert_eq!(greets(&fixture.step), "hello first");

  write(
    &fixture.step.join("src/lib.rs"),
    "include!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));\n\npub fn greeting() -> &'static str {\n  \"hi\"\n}\n",
  );
  write(&fixture.step.join("assets/generated.txt"), "second\n");
  for command in ["build", "clippy"] {
    let rebuilt = cargo_ok(&fixture.step, command);
    assert!(!rebuilt.contains("Fresh warm"), "the step's change is built: {rebuilt}");
  }
  assert_eq!(greets(&fixture.step), "hi second");

  assert_eq!(greets(&fixture.run), "hello first");
  let after = snapshot(&fixture.run.join("target"));
  let changed: Vec<&PathBuf> = before
    .iter()
    .filter(|(path, file)| after.get(*path) != Some(file))
    .map(|(path, _)| path)
    .collect();
  assert!(
    changed.is_empty(),
    "the step's builds changed the run's files {changed:?}"
  );
  assert_eq!(after.len(), before.len());
  assert!(
    !cargo_ok(&fixture.run, "build").contains("Compiling warm"),
    "the run's build stays fresh"
  );
}

#[test]
fn removing_the_step_worktree_leaves_the_run_build() {
  let fixture = Fixture::seeded();
  cargo_ok(&fixture.step, "build");
  let before = snapshot(&fixture.run.join("target"));
  assert_eq!(
    git(&fixture.step, &["status", "--porcelain", "--untracked-files=all"]),
    ""
  );

  Worktrees::new(fixture.repo.path())
    .expect("worktrees")
    .remove_merged_step(fixture.execution, "orchester/run-1--check-1")
    .expect("the step's worktree removed");

  assert!(!fixture.step.exists());
  assert_eq!(snapshot(&fixture.run.join("target")), before);
  assert!(
    !cargo_ok(&fixture.run, "build").contains("Compiling warm"),
    "the run's build stays fresh"
  );
  assert_eq!(greets(&fixture.run), "hello first");
}
