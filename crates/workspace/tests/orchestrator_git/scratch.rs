//! Real git in folders of the tests' own under the worktree's `.tmp/`: git's settings are a file of each test's own
//! (`GIT_CONFIG_GLOBAL`, `GIT_CONFIG_NOSYSTEM`, the author named there), git looks for no repository above each test's
//! folder (`GIT_CEILING_DIRECTORIES`) and goes through no proxy. A repository `kontur` on `main` with one commit,
//! and a bare `origin` it tracks.

use std::{
  fs,
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  orchestrator::git::GitChange,
};
use orchester_exec::{ProcessRunner, programs::Programs};
use orchester_workspace::{
  app_mcp::ToolError,
  orchestrator::git::{Called, apply, call},
};
use serde_json::Value;
use tempfile::TempDir;

/// git with the settings file `global` only, looking for no repository above `ceiling`.
struct Isolated {
  global: PathBuf,
  ceiling: PathBuf,
}

impl CommandRunner for Isolated {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let mut isolated = command
      .clone()
      .with_env("GIT_CONFIG_GLOBAL", &self.global.to_string_lossy())
      .with_env("GIT_CONFIG_NOSYSTEM", "1")
      .with_env("GIT_CEILING_DIRECTORIES", &self.ceiling.to_string_lossy())
      .with_env("NO_PROXY", "*");
    for variable in [
      "GIT_AUTHOR_NAME",
      "GIT_AUTHOR_EMAIL",
      "GIT_COMMITTER_NAME",
      "GIT_COMMITTER_EMAIL",
      "EMAIL",
      "HTTP_PROXY",
      "HTTPS_PROXY",
      "ALL_PROXY",
      "http_proxy",
      "https_proxy",
      "all_proxy",
    ] {
      isolated = isolated.without_env(variable);
    }
    ProcessRunner::new(Duration::from_secs(60)).run(&isolated)
  }
}

pub struct Scratch {
  pub dir: TempDir,
  runner: Arc<Isolated>,
  pub programs: Programs,
}

impl Scratch {
  /// `kontur` on `main` with `README.md` committed, tracking `origin/main` of a bare `origin`.
  pub fn new() -> Self {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orchestrator-git/tests");
    fs::create_dir_all(&root).expect("a scratch folder");
    let dir = tempfile::tempdir_in(root).expect("a folder of the test's own");
    let global = dir.path().join("gitconfig");
    fs::write(
      &global,
      "[user]\n\tname = Test Author\n\temail = author@example.com\n\tuseConfigOnly = true\n[core]\n\tautocrlf = \
       false\n[init]\n\tdefaultBranch = main\n",
    )
    .expect("git's settings");
    let runner = Arc::new(Isolated {
      global,
      ceiling: dir.path().to_path_buf(),
    });
    let programs = Programs::with_runner(runner.clone());
    let scratch = Self { dir, runner, programs };
    scratch.git_in(scratch.dir.path(), &["init", "--bare", "origin.git"]);
    scratch.git_in(scratch.dir.path(), &["init", "kontur"]);
    scratch.write("README.md", "Kontur\n");
    scratch.git(&["add", "-A"]);
    scratch.git(&["commit", "-m", "Initial commit"]);
    let origin = scratch.origin();
    scratch.git(&["remote", "add", "origin", &origin.to_string_lossy()]);
    scratch.git(&["push", "-u", "origin", "main"]);
    scratch
  }

  pub fn root(&self) -> PathBuf {
    self.dir.path().join("kontur")
  }

  pub fn origin(&self) -> PathBuf {
    self.dir.path().join("origin.git")
  }

  pub fn write(&self, name: &str, text: &str) {
    let path = self.root().join(name);
    fs::create_dir_all(path.parent().expect("a file in the folder")).expect("its folder");
    fs::write(path, text).expect("a file");
  }

  pub fn read(&self, name: &str) -> String {
    fs::read_to_string(self.root().join(name)).expect("the file")
  }

  /// `git <args>` in the repository, which must work; its output.
  pub fn git(&self, args: &[&str]) -> String {
    self.git_in(&self.root(), args)
  }

  pub fn git_in(&self, dir: &Path, args: &[&str]) -> String {
    let output = self.try_git_in(dir, args);
    assert!(output.success(), "git {args:?}: {}", output.stderr);
    output.stdout.trim().to_owned()
  }

  pub fn try_git_in(&self, dir: &Path, args: &[&str]) -> CommandOutput {
    self
      .runner
      .run(&Command::new(Path::new("git"), args.iter().copied()).with_current_dir(dir))
      .expect("git runs")
  }

  /// A commit of `name` holding `text`, with `message`.
  pub fn commit(&self, name: &str, text: &str, message: &str) {
    self.write(name, text);
    self.git(&["add", "-A"]);
    self.git(&["commit", "-m", message]);
  }

  /// The commit `rev` names.
  pub fn head(&self, rev: &str) -> String {
    self.git(&["rev-parse", rev])
  }

  /// The chat's git tool `name` with `arguments` in this repository, `writes` the person's switch.
  pub fn call(&self, name: &str, arguments: &Value, writes: bool) -> Result<Called, ToolError> {
    let Value::Object(arguments) = arguments else {
      panic!("arguments are an object");
    };
    call(&self.programs, &self.root(), (name, arguments), writes)
  }

  /// A read's answer.
  pub fn read_tool(&self, name: &str, arguments: &Value) -> Value {
    match self.call(name, arguments, false) {
      Ok(Called::Read(read)) => read,
      other => panic!("{name} reads: {other:?}"),
    }
  }

  /// A write's answer, or why it was refused, and what ran.
  pub fn write_tool(&self, name: &str, arguments: &Value) -> (Result<Value, ToolError>, GitChange) {
    match self.call(name, arguments, true) {
      Ok(Called::Wrote { result, change }) => (result, change),
      other => panic!("{name} writes: {other:?}"),
    }
  }

  /// A write that worked.
  pub fn wrote(&self, name: &str, arguments: &Value) -> Value {
    let (result, change) = self.write_tool(name, arguments);
    let told = result.unwrap_or_else(|error| panic!("{name}: {error:?}"));
    assert!(!change.commands.is_empty(), "{name} keeps what ran");
    told
  }

  /// A proposal of what can lose work.
  pub fn proposed(&self, arguments: &Value) -> GitChange {
    match self.call("git_propose", arguments, false) {
      Ok(Called::Proposed(change)) => change,
      other => panic!("a proposal: {other:?}"),
    }
  }

  /// The person applies `change`.
  pub fn apply(&self, change: &GitChange) -> Result<Value, ToolError> {
    apply(&self.programs, &self.root(), change)
  }
}

pub fn code(error: &ToolError) -> &str {
  match error {
    ToolError::Refused { code, .. } => code,
    other => panic!("a refusal, not {other:?}"),
  }
}

/// The refusal's code of a call that was refused.
pub fn refused(called: Result<Called, ToolError>) -> String {
  match called {
    Err(error) => code(&error).to_owned(),
    Ok(Called::Wrote { result: Err(error), .. }) => code(&error).to_owned(),
    other => panic!("a refusal, not {other:?}"),
  }
}
