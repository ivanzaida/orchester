//! Real temporary git repositories laid out as the runner leaves them, and runs to integrate.

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  io::ErrorKind,
  num::NonZeroU32,
  path::{Path, PathBuf},
  process,
  time::Duration,
};

use orchester_core::{
  command::{Command, CommandError, CommandOutput, CommandRunner},
  id::{ConnectionId, ResourceId, RunId, StepExecutionId, TaskId, VersionId},
  name::Name,
  permission::StepPermissions,
  project::{ProjectKey, TaskKey},
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Step, StepKey, StepKind, Version,
    VersionNumber,
  },
  revision::Revision,
  run::{BranchName, InterruptCause, NewRun, PublishedVersions, Run, RunNumber, RunSnapshot},
  time::Timestamp,
  tokens::TokenBudget,
};
use tempfile::TempDir;

/// The branch of run #3 of ORC-12.
pub const RUN_BRANCH: &str = "orchester/orc-12-run-3";

/// Runs programs as child processes.
pub struct Processes;

impl CommandRunner for Processes {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let mut child = process::Command::new(command.program());
    child.args(command.args()).envs(command.env().iter().cloned());
    if let Some(directory) = command.current_dir() {
      child.current_dir(directory);
    }
    let output = child.output().map_err(|error| match error.kind() {
      ErrorKind::NotFound => CommandError::NotFound,
      _ => CommandError::Failed(error.to_string()),
    })?;
    Ok(CommandOutput {
      code: output.status.code(),
      stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
      stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
  }
}

pub fn git_program() -> PathBuf {
  PathBuf::from("git")
}

pub fn now() -> Timestamp {
  Timestamp::from_unix_millis(1_700_000_000_000)
}

pub fn task() -> TaskKey {
  TaskKey::new(
    ProjectKey::new("ORC").expect("valid key"),
    NonZeroU32::new(12).expect("positive"),
  )
}

/// A repository on `main` with one commit, `.orchester/` excluded as a project excludes it.
pub struct Repo {
  _dir: TempDir,
  pub root: PathBuf,
}

impl Repo {
  pub fn new() -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-core-integration-tests");
    fs::create_dir_all(&scratch).expect("private worktree scratch");
    let dir = tempfile::tempdir_in(scratch).expect("temporary directory");
    let root = dir.path().join("repo");
    fs::create_dir(&root).expect("repository folder");
    let repo = Self { _dir: dir, root };
    repo.git(&["init", "--quiet", "--initial-branch=main"]);
    repo.git(&["config", "user.name", "Orchester Test"]);
    repo.git(&["config", "user.email", "test@orchester.invalid"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    let exclude = repo.root.join(".git").join("info").join("exclude");
    fs::write(&exclude, "/.orchester/\n").expect("exclude .orchester");
    repo.commit(&repo.root, "README.md", "hello\n", "initial");
    repo
  }

  /// `git <args>` in the repository, which must succeed; its trimmed output.
  pub fn git(&self, args: &[&str]) -> String {
    git_in(&self.root, args)
  }

  /// Writes `file` in `dir` and commits it there; the new commit.
  pub fn commit(&self, dir: &Path, file: &str, content: &str, message: &str) -> String {
    fs::write(dir.join(file), content).expect("write file");
    git_in(dir, &["add", "--all"]);
    git_in(dir, &["commit", "--quiet", "-m", message]);
    git_in(dir, &["rev-parse", "HEAD"])
  }

  pub fn head(&self) -> String {
    self.git(&["rev-parse", "HEAD"])
  }

  /// A worktree at `.orchester/worktrees/<id>` on a new `branch` from `base`, as the runner adds.
  pub fn worktree(&self, id: &str, branch: &str, base: &str) -> PathBuf {
    let path = self.root.join(".orchester").join("worktrees").join(id);
    let target = path.to_str().expect("a Unicode temporary path");
    self.git(&["worktree", "add", "--quiet", "-b", branch, target, base]);
    path
  }

  pub fn branch_exists(&self, branch: &str) -> bool {
    process::Command::new("git")
      .arg("-C")
      .arg(&self.root)
      .args(["show-ref", "--verify", "--quiet", &format!("refs/heads/{branch}")])
      .status()
      .expect("run git")
      .success()
  }

  pub fn is_clean(&self) -> bool {
    self.git(&["status", "--porcelain"]).is_empty()
  }
}

pub fn git_in(dir: &Path, args: &[&str]) -> String {
  let output = process::Command::new("git")
    .arg("-C")
    .arg(dir)
    .args(args)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("valid key")
}

fn step(value: &str, profile: ResourceId, depends_on: &[&str]) -> Step {
  Step {
    key: key(value),
    name: Name::new(value).expect("valid name"),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: depends_on
      .iter()
      .map(|dependency| key(dependency))
      .collect::<BTreeSet<_>>(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

fn version<B>(body: B) -> Version<B> {
  Version::new(VersionId::generate(), VersionNumber::new(NonZeroU32::MIN), body, now())
}

/// Implement, then Review and Checks beside each other.
fn snapshot() -> RunSnapshot {
  let builder = ResourceId::generate();
  let pipeline = Pipeline::new(vec![
    step("implement", builder, &[]),
    step("review", builder, &["implement"]),
    step("checks", builder, &["implement"]),
  ])
  .expect("valid pipeline");
  let profile = AgentProfile::new(
    Name::new("Builder").expect("valid role"),
    String::new(),
    PromptText::new("Work.").expect("valid prompt"),
    ConnectionId::generate(),
    ModelId::new("model-b").expect("valid model"),
    ProviderSettings::default(),
  );
  let profiles: BTreeMap<ResourceId, Version<AgentProfile>> = BTreeMap::from([(builder, version(profile))]);
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &BTreeMap::new(),
    instructions: &BTreeMap::new(),
    skills: &BTreeMap::new(),
  };
  RunSnapshot::build(ResourceId::generate(), &version(pipeline), published).expect("every reference is published")
}

/// How a test run ends.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Ending {
  Succeeded,
  Cancelled,
  /// Still running.
  Running,
  /// Interrupted when a person chose Quit now.
  Interrupted,
}

/// Run #3 of ORC-12 on [`RUN_BRANCH`], started at `base` with the checkout on `base_branch`.
pub fn run(base: &str, base_branch: Option<&str>, ending: Ending) -> Run {
  let new = NewRun {
    task: TaskId::generate(),
    snapshot: snapshot(),
    branch: BranchName::new(RUN_BRANCH).expect("valid branch"),
    base: Revision::new(base).expect("a full revision"),
    base_branch: base_branch.map(|branch| BranchName::new(branch).expect("valid branch")),
    budget: Some(TokenBudget::new(1_400_000).expect("a budget")),
    max_duration: Some(Duration::from_secs(30 * 60)),
  };
  let number = RunNumber::new(NonZeroU32::new(3).expect("positive"));
  let (mut run, _) = Run::create(RunId::generate(), number, new, now()).expect("a valid run");
  run.start(now()).expect("the run starts");
  match ending {
    Ending::Succeeded => {
      for step in ["implement", "review", "checks"] {
        let execution = execution(&run, step);
        run.start_step(execution, now()).expect("the step starts");
        run.complete_step(execution, now()).expect("the step passes");
      }
    }
    Ending::Cancelled => {
      run.cancel(now()).expect("the run is cancelled");
    }
    Ending::Running => {}
    Ending::Interrupted => {
      run
        .interrupt(InterruptCause::QuitNow, now())
        .expect("the run is interrupted");
    }
  }
  run
}

/// The execution of `step` in the run's attempt.
pub fn execution(run: &Run, step: &str) -> StepExecutionId {
  run
    .latest_execution(&key(step))
    .expect("the step has an execution")
    .id()
}
