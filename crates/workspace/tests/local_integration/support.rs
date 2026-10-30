use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use orchester_core::{
  id::{IdempotencyKey, ProjectId, RunId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{CommandAction, CommandArg, CommandProgram, Pipeline, PromptText, Slug, Step, StepKey, StepKind, Tool},
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, RunSnapshot},
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::local_integration::LocalIntegration;

pub struct Fixture {
  pub repo: tempfile::TempDir,
  pub run: RunId,
  pub task_root: PathBuf,
}

impl Fixture {
  pub fn new(check_fails: bool) -> Self {
    Self::named(check_fails, "workspace-")
  }
  pub fn named(check_fails: bool, prefix: &str) -> Self {
    Self::branched(check_fails, prefix, "task")
  }
  /// A fixture whose run delivers on `branch`.
  pub fn branched(check_fails: bool, prefix: &str, branch: &str) -> Self {
    Self::recorded(check_fails, prefix, branch, None)
  }
  /// [`Self::branched`], whose run writes `task.txt` as `checkpointed` and records that as a checkpoint when it names
  /// one.
  pub fn recorded(check_fails: bool, prefix: &str, branch: &str, checkpointed: Option<&str>) -> Self {
    Self::worked(check_fails, (prefix, branch), |_, task_root| {
      fs::write(task_root.join("task.txt"), checkpointed.unwrap_or("delivered task\n")).expect("task edit");
      git(task_root, &["commit", "-am", "task work"]);
      checkpointed
        .map(|_| git(task_root, &["rev-parse", "HEAD"]))
        .into_iter()
        .collect()
    })
  }

  /// A fixture whose run's branch is what `work` makes of it, given main's checkout and the run's worktree, with the
  /// commits it gives recorded as the run's checkpoints, in order.
  pub fn worked(
    check_fails: bool,
    (prefix, branch): (&str, &str),
    work: impl FnOnce(&Path, &Path) -> Vec<String>,
  ) -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    let repo = tempfile::Builder::new()
      .prefix(prefix)
      .tempdir_in(scratch)
      .expect("repo");
    git(repo.path(), &["init", "-b", "main"]);
    git(repo.path(), &["config", "user.name", "Local integration test"]);
    git(repo.path(), &["config", "user.email", "test@example.invalid"]);
    git(repo.path(), &["config", "core.autocrlf", "false"]);
    fs::write(repo.path().join(".gitignore"), ".orchester/\nignored.txt\n").expect("ignore");
    fs::write(repo.path().join("local.txt"), "base\n").expect("local");
    fs::write(repo.path().join("task.txt"), "task base\n").expect("task");
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "base"]);
    let base = git(repo.path(), &["rev-parse", "HEAD"]);
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("TEST").expect("key"),
      name("Test project"),
    );
    let mut db = ProjectDb::create(repo.path(), project).expect("project");
    let snapshot = check_snapshot(&mut db, check_fails);
    let task = db
      .create_tasks(
        IdempotencyKey::generate(),
        vec![TaskDetails {
          title: name("Integrate changes"),
          brief: String::new(),
          acceptance_criteria: Vec::new(),
          pipeline: None,
          limits: TaskLimits::NONE,
        }],
        at(5),
      )
      .expect("task")
      .remove(0);
    let mut run = db
      .create_run(
        IdempotencyKey::generate(),
        NewRun {
          task: task.id(),
          snapshot,
          branch: BranchName::new(branch).expect("branch"),
          base: Revision::new(&base).expect("revision"),
          base_branch: Some(BranchName::new("main").expect("branch")),
          base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
          budget: None,
          max_duration: None,
        },
        at(6),
      )
      .expect("run");
    let events = run.start(at(7)).expect("start");
    db.append_events(&run, &events).expect("events");
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let events = run.start_step(execution, at(8)).expect("step");
    db.append_events(&run, &events).expect("events");
    let task_root = repo.path().join(".orchester/worktrees").join(run.id().to_string());
    fs::create_dir_all(task_root.parent().expect("parent")).expect("parent");
    git(
      repo.path(),
      &["worktree", "add", "-b", branch, task_root.to_str().expect("path")],
    );
    for checkpoint in work(repo.path(), &task_root) {
      let checkpoint = Revision::new(&checkpoint).expect("revision");
      let events = run.record_checkpoint(execution, checkpoint, at(9)).expect("checkpoint");
      db.append_events(&run, &events).expect("events");
    }
    let events = run.complete_step(execution, at(9)).expect("pass");
    db.append_events(&run, &events).expect("events");
    Self {
      repo,
      run: run.id(),
      task_root,
    }
  }

  pub fn root(&self) -> &Path {
    self.repo.path()
  }
  pub fn open(&self) -> LocalIntegration {
    LocalIntegration::open(self.root(), self.run).expect("open")
  }
}

pub fn git(root: &Path, args: &[&str]) -> String {
  let output = Command::new("git").args(args).current_dir(root).output().expect("git");
  assert!(output.status.success(), "command refused: {args:?}");
  String::from_utf8(output.stdout).expect("text").trim().to_owned()
}
fn name(value: &str) -> Name {
  Name::new(value).expect("name")
}
fn arg(value: &str) -> CommandArg {
  CommandArg::new(value).expect("arg")
}
fn at(value: i64) -> Timestamp {
  Timestamp::from_unix_millis(value)
}
fn check_snapshot(db: &mut ProjectDb, check_fails: bool) -> RunSnapshot {
  let action = CommandAction::new(
    CommandProgram::new("git").expect("program"),
    if check_fails {
      vec![arg("diff"), arg("--exit-code"), arg("HEAD^"), arg("HEAD")]
    } else {
      vec![arg("status"), arg("--porcelain")]
    },
  );
  let tool = db
    .create_resource(
      Slug::new("check-tool").expect("slug"),
      name("Check"),
      &Tool::command("", action),
      at(1),
    )
    .expect("tool");
  let tool_version = db.publish_draft::<Tool>(tool.id(), at(2)).expect("publish");
  let pipeline = Pipeline::new(vec![Step {
    key: StepKey::new("check").expect("key"),
    name: name("Check"),
    kind: StepKind::Tool { tool: Some(tool.id()) },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }])
  .expect("pipeline");
  let resource = db
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, at(3))
    .expect("pipeline");
  let version = db.publish_draft::<Pipeline>(resource.id(), at(4)).expect("publish");
  let tools = BTreeMap::from([(tool.id(), tool_version)]);
  let published = PublishedVersions {
    profiles: &BTreeMap::new(),
    tools: &tools,
    instructions: &BTreeMap::new(),
    skills: &BTreeMap::new(),
  };
  RunSnapshot::build(resource.id(), &version, published).expect("snapshot")
}
