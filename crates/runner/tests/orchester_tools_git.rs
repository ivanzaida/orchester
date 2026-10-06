//! Orchester's git tools against a real git: a nested repository keeps a configuration of its own, whose clean
//! filter a plain `git status`, or a second `git add .` once it is staged, runs. The tools never enter one:
//! `git_add` stages named files only, none in a nested repository, and every other call has submodules off.
//! Needs `git`. Run with `cargo test -p orchester-runner --test orchester_tools_git orchester_tools_git`.

use std::{
  ffi::OsString,
  fs,
  path::{Path, PathBuf},
  process::Command as Process,
  time::{Duration, SystemTime},
};

use orchester_core::{
  connection::invocation::Platform,
  orchester_tools::{OutsideReads, StepTools, ToolCall, ToolCallError, ToolContext, ToolResult, ToolWork},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess, category::CommandScope,
  },
  resource::CommandEnvironment,
};
use orchester_exec::run_capped;
use serde_json::{Value, json};

fn git(folder: &Path, args: &[&str]) -> String {
  let output = Process::new("git")
    .args(args)
    .current_dir(folder)
    .output()
    .expect("git runs");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8_lossy(&output.stdout).into_owned()
}

fn identify(folder: &Path) {
  git(folder, &["config", "user.name", "Orchester Test"]);
  git(folder, &["config", "user.email", "test@orchester.invalid"]);
  git(folder, &["config", "commit.gpgsign", "false"]);
}

/// A repository, and in it the folder `nested` of a repository whose own configuration names a clean filter
/// that creates `marker` whenever git hashes its files.
struct Probe {
  _root: tempfile::TempDir,
  repository: PathBuf,
  marker: PathBuf,
  hooks: PathBuf,
  inherited: Vec<(OsString, OsString)>,
  environment: CommandEnvironment,
}

/// How the nested repository came to be.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Nested {
  /// Made by the agent and not staged.
  Fresh,
  /// Staged as a gitlink by an earlier plain `git add`.
  Staged,
  /// A submodule a person added (`git submodule add`), its filter set in its own configuration.
  Submodule,
}

impl Probe {
  fn new(nested: Nested) -> Self {
    let root = tempfile::tempdir().expect("a temporary folder");
    let (repository, marker, hooks) = (
      root.path().join("repository"),
      root.path().join("marker"),
      root.path().join("no-hooks"),
    );
    fs::create_dir_all(&repository).expect("a folder");
    fs::create_dir_all(&hooks).expect("a hooks folder");
    git(&repository, &["init", "--quiet"]);
    identify(&repository);
    fs::write(repository.join("README.md"), "hi\n").expect("a file");
    git(&repository, &["add", "README.md"]);
    git(&repository, &["commit", "--quiet", "-m", "initial"]);
    let source = if nested == Nested::Submodule {
      root.path().join("source")
    } else {
      repository.join("nested")
    };
    fs::create_dir_all(&source).expect("a folder");
    git(&source, &["init", "--quiet"]);
    identify(&source);
    fs::write(source.join(".gitattributes"), "* filter=evil\n").expect("attributes");
    fs::write(source.join("a.txt"), "aaaa\n").expect("a file");
    git(&source, &["add", "."]);
    git(&source, &["commit", "--quiet", "-m", "nested"]);
    let source_text = source.to_string_lossy().into_owned();
    match nested {
      Nested::Fresh => {}
      Nested::Staged => {
        git(&repository, &["add", "nested"]);
      }
      Nested::Submodule => {
        git(
          &repository,
          &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "--quiet",
            &source_text,
            "nested",
          ],
        );
      }
    }
    let touch = format!("touch '{}'; cat", marker.to_string_lossy().replace('\\', "/"));
    git(&repository.join("nested"), &["config", "filter.evil.clean", &touch]);
    let probe = Self {
      _root: root,
      repository,
      marker,
      hooks,
      inherited: std::env::vars_os().collect(),
      environment: CommandEnvironment::default(),
    };
    probe.forget_marker();
    probe
  }

  /// Removes the marker and makes git look at the nested file's content again.
  fn forget_marker(&self) {
    if self.marker.exists() {
      fs::remove_file(&self.marker).expect("the marker goes");
    }
    let file = self.repository.join("nested").join("a.txt");
    fs::write(&file, "aaaa\n").expect("rewritten");
    fs::File::options()
      .write(true)
      .open(&file)
      .and_then(|opened| opened.set_modified(SystemTime::now() + Duration::from_secs(5)))
      .expect("a new stamp");
  }

  /// The answer to a call of `tool` with `arguments`, or why it does not run; the nested filter must not run.
  fn call(&self, tool: &str, arguments: &Value) -> Result<ToolResult, ToolCallError> {
    self.forget_marker();
    let reads = OutsideReads::new(CommandScope::on_host(&self.repository, &[]), &[]);
    let git_dir = self.repository.join(".git");
    let context = ToolContext {
      worktree: &self.repository,
      inherited: &self.inherited,
      project: &self.environment,
      outside: &reads,
      project_tools: &[],
      platform: Platform::current(),
      git_dir: &git_dir,
      hooks: &self.hooks,
    };
    let permissions = AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::Allowed(AllowedCommands::default()),
      NetworkAccess::Off,
    );
    let call = ToolCall::parse(tool, arguments, &StepTools::for_step(&permissions, &[])).expect("valid");
    let answer = match call.work(&context)? {
      ToolWork::Run(program) => program.finished(run_capped(program.command(), program.timeout(), program.cap())),
      ToolWork::Done(result) => result,
      ToolWork::Files(_) | ToolWork::Symbols(_) => panic!("{tool} is a git tool"),
    };
    assert!(!answer.is_error(), "{tool}: {}", answer.text());
    assert!(
      !self.marker.exists(),
      "{tool} {arguments} ran the nested repository's filter"
    );
    Ok(answer)
  }
}

/// The probe of the review of 2026-10-05: a plain `git status` runs a staged nested repository's filter; none
/// of the git tools does, and staging into it is refused.
#[test]
fn orchester_tools_git_never_enter_a_nested_repository() {
  let probe = Probe::new(Nested::Staged);
  git(&probe.repository, &["status", "--short"]);
  assert!(probe.marker.exists(), "a plain git status runs the nested filter");
  for (tool, arguments) in [
    ("git_status", json!({})),
    ("git_diff", json!({})),
    ("git_diff", json!({"staged": true})),
    ("git_log", json!({})),
    ("git_show", json!({"revision": "HEAD"})),
    ("git_add", json!({"paths": ["README.md"]})),
    ("git_commit", json!({"message": "Stage the nested repository"})),
    ("git_show", json!({"revision": "HEAD"})),
  ] {
    assert!(probe.call(tool, &arguments).is_ok(), "{tool}");
  }
  assert_eq!(
    probe.call("git_add", &json!({"paths": ["nested/a.txt"]})).err(),
    Some(ToolCallError::NestedRepository("nested/a.txt".to_owned()))
  );
}

/// `git add .` twice stages a fresh nested repository, then runs its filter: `git_add` takes no folder, so the
/// nested repository is never staged, also not through a folder that holds it.
#[test]
fn orchester_tools_git_add_never_stages_a_fresh_nested_repository() {
  let probe = Probe::new(Nested::Fresh);
  fs::create_dir_all(probe.repository.join("vendor").join("lib").join(".git")).expect("another nested repository");
  for _ in 0..2 {
    for path in [".", "nested", "vendor"] {
      assert_eq!(
        probe.call("git_add", &json!({ "paths": [path] })).err(),
        Some(ToolCallError::Folder(path.to_owned()))
      );
    }
  }
  fs::write(probe.repository.join("notes.md"), "notes\n").expect("a file");
  assert!(probe.call("git_add", &json!({"paths": ["notes.md"]})).is_ok());
  assert!(
    git(&probe.repository, &["ls-files", "--stage"])
      .lines()
      .all(|entry| !entry.starts_with("160000"))
  );
}

/// A person's real submodule: its filter runs for none of the tools, and nothing is staged into it.
#[test]
fn orchester_tools_git_leave_a_real_submodule_alone() {
  let probe = Probe::new(Nested::Submodule);
  fs::write(probe.repository.join("notes.md"), "notes\n").expect("a file");
  for (tool, arguments) in [
    ("git_status", json!({})),
    ("git_add", json!({"paths": ["notes.md", ".gitmodules"]})),
    ("git_diff", json!({"staged": true})),
    ("git_commit", json!({"message": "Notes beside the submodule"})),
    ("git_log", json!({})),
  ] {
    assert!(probe.call(tool, &arguments).is_ok(), "{tool}");
  }
  assert_eq!(
    probe.call("git_add", &json!({"paths": ["."]})).err(),
    Some(ToolCallError::Folder(".".to_owned()))
  );
}
