//! Orchester's git tools against a real git: a nested repository the agent made and staged keeps a
//! configuration of its own, whose clean filter a plain `git status` runs; the tools never enter it, and
//! `git_add` refuses to stage one. Needs `git`. Run with `cargo test -p orchester-runner --test
//! orchester_tools_git orchester_tools_git`.

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command as Process,
};

use orchester_core::{
  connection::invocation::Platform,
  orchester_tools::{OutsideReads, StepTools, ToolCall, ToolCallError, ToolContext, ToolWork},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess, category::CommandScope,
  },
  resource::CommandEnvironment,
};
use orchester_exec::run_capped;
use serde_json::json;

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

/// A repository with a nested repository staged as a gitlink, whose own configuration names a clean filter
/// that creates `marker` whenever git hashes the nested repository's files.
struct Probe {
  _root: tempfile::TempDir,
  repository: PathBuf,
  marker: PathBuf,
  hooks: PathBuf,
}

impl Probe {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("a temporary folder");
    let repository = root.path().join("repository");
    let nested = repository.join("nested");
    let marker = root.path().join("marker");
    let hooks = root.path().join("no-hooks");
    fs::create_dir_all(&nested).expect("folders");
    fs::create_dir_all(&hooks).expect("a hooks folder");
    for folder in [&repository, &nested] {
      git(folder, &["init", "--quiet"]);
      git(folder, &["config", "user.name", "Orchester Test"]);
      git(folder, &["config", "user.email", "test@orchester.invalid"]);
      git(folder, &["config", "commit.gpgsign", "false"]);
    }
    fs::write(repository.join("README.md"), "hi\n").expect("a file");
    git(&repository, &["add", "README.md"]);
    git(&repository, &["commit", "--quiet", "-m", "initial"]);
    let touch = format!("touch '{}'; cat", marker.to_string_lossy().replace('\\', "/"));
    git(&nested, &["config", "filter.evil.clean", &touch]);
    fs::write(nested.join(".gitattributes"), "* filter=evil\n").expect("attributes");
    fs::write(nested.join("a.txt"), "aaaa\n").expect("a file");
    git(&nested, &["add", "."]);
    git(&nested, &["commit", "--quiet", "-m", "nested"]);
    git(&repository, &["add", "nested"]);
    let probe = Self {
      _root: root,
      repository,
      marker,
      hooks,
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
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    fs::File::options()
      .write(true)
      .open(&file)
      .and_then(|opened| opened.set_modified(later))
      .expect("a new stamp");
  }
}

fn writing() -> StepTools {
  let permissions = AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::Off,
  );
  StepTools::for_step(&permissions, &[])
}

/// The probe of the review of 2026-10-05: a plain `git status` runs the nested repository's filter; none of the
/// git tools does, and staging the nested repository is refused.
#[test]
fn orchester_tools_git_never_enter_a_nested_repository() {
  let probe = Probe::new();
  git(&probe.repository, &["status", "--short"]);
  assert!(probe.marker.exists(), "a plain git status runs the nested filter");
  let reads = OutsideReads::new(CommandScope::on_host(&probe.repository, &[]), &[]);
  let environment = CommandEnvironment::default();
  let inherited: Vec<_> = std::env::vars_os().collect();
  let context = ToolContext {
    worktree: &probe.repository,
    inherited: &inherited,
    project: &environment,
    outside: &reads,
    project_tools: &[],
    platform: Platform::current(),
    git_dir: &probe.repository.join(".git"),
    hooks: &probe.hooks,
  };
  let tools = writing();
  for (tool, arguments) in [
    ("git_status", json!({})),
    ("git_diff", json!({})),
    ("git_diff", json!({"staged": true})),
    ("git_log", json!({})),
    ("git_show", json!({"revision": "HEAD"})),
    ("git_commit", json!({"message": "Stage the nested repository"})),
    ("git_show", json!({"revision": "HEAD"})),
  ] {
    probe.forget_marker();
    let call = ToolCall::parse(tool, &arguments, &tools).expect("valid");
    let Ok(ToolWork::Run(program)) = call.work(&context) else {
      panic!("{tool} runs git");
    };
    let result = program.finished(run_capped(program.command(), program.timeout(), program.cap()));
    assert!(!result.is_error(), "{tool}: {}", result.text());
    assert!(
      !probe.marker.exists(),
      "{tool} {arguments} ran the nested repository's filter"
    );
  }
  for path in ["nested", "nested/a.txt"] {
    let call = ToolCall::parse("git_add", &json!({ "paths": [path] }), &tools).expect("valid");
    assert_eq!(
      call.work(&context),
      Err(ToolCallError::NestedRepository(path.to_owned()))
    );
  }
}
