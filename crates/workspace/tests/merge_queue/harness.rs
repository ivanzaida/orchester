//! A project in a git repository on `main` for the merge queue's tests: a Builder profile, pipelines Implement → Check
//! by how they deliver, runs that passed both steps in worktrees of their own, and a check whose program the tests
//! answer for while git runs as a process.

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::{Path, PathBuf},
  process::Command,
  sync::Arc,
  time::Duration,
};

use orchester_core::{
  command::{Command as Program, CommandError, CommandOutput, CommandRunner},
  id::{ConnectionId, IdempotencyKey, ProjectId, ResourceId},
  merge_queue::{QueueEvent, QueueState},
  name::Name,
  permission::{AgentPermissions, CommandPermission, NetworkAccess, StepPermissions},
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, CommandAction, CommandProgram, DeliverMode, DeliverStep, MergeMethod, ModelId, Pipeline, PromptText,
    ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind, Tool,
  },
  revision::Revision,
  run::{Answerer, BranchName, NewRun, PublishedVersions, ReviewNote, Run, RunSnapshot},
  store::{ResourceStore, RunStore, TaskStore},
  task::{Task, TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_exec::{ProcessRunner, programs::Programs};
use orchester_ipc::Command as RunnerCommand;
use orchester_store::ProjectDb;
use orchester_workspace::merge_queue::{self, Advance};
use parking_lot::Mutex;

/// `git <args>` in `root`, which must succeed; its trimmed output.
pub fn git(root: &Path, args: &[&str]) -> String {
  let output = Command::new("git").args(args).current_dir(root).output().expect("git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout).expect("text").trim().to_owned()
}

fn name(value: &str) -> Name {
  Name::new(value).expect("name")
}

pub fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

/// What the check does when it runs: its exit code, after `side` (e.g. a commit on `main` meanwhile).
type CheckScript = Box<dyn FnMut() -> i32 + Send>;

/// Runs git as a process and answers for the check's program as its script says.
struct Answered {
  program: PathBuf,
  script: Mutex<CheckScript>,
  checks: Mutex<u32>,
}

impl CommandRunner for Answered {
  fn run(&self, command: &Program) -> Result<CommandOutput, CommandError> {
    if command.program() != self.program {
      return ProcessRunner::new(Duration::from_secs(120)).run(command);
    }
    *self.checks.lock() += 1;
    let code = (self.script.lock())();
    Ok(CommandOutput {
      code: Some(code),
      stdout: format!("checked, exit {code}\n"),
      stderr: String::new(),
    })
  }
}

/// A project in a git repository on `main` with a Builder profile and pipelines Implement → Check, one per way of
/// delivering: approve to merge, auto-merge with a merge commit, auto-merge squashed, and none.
pub struct Repo {
  dir: tempfile::TempDir,
  /// Where the check's program lives, outside the repository.
  _tools: tempfile::TempDir,
  db: ProjectDb,
  pipelines: BTreeMap<&'static str, RunSnapshot>,
  answered: Arc<Answered>,
}

impl Repo {
  pub fn new() -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-19-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    let dir = tempfile::Builder::new()
      .prefix("merge-queue-")
      .tempdir_in(&scratch)
      .expect("repo");
    let tools = tempfile::Builder::new()
      .prefix("merge-queue-tools-")
      .tempdir_in(&scratch)
      .expect("tools");
    let program = tools.path().join("check.cmd");
    fs::write(&program, "").expect("program");
    let root = dir.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.name", "Merge queue test"]);
    git(root, &["config", "user.email", "test@example.invalid"]);
    git(root, &["config", "core.autocrlf", "false"]);
    fs::write(root.join(".gitignore"), ".orchester/\n").expect("ignore");
    fs::write(root.join("shared.txt"), "base\n").expect("shared");
    fs::write(root.join("other.txt"), "base\n").expect("other");
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "base"]);
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("ORC").expect("key"),
      name("Kontur"),
    );
    let mut db = ProjectDb::create(root, project).expect("project");
    let pipelines = pipelines(&mut db, &program);
    let answered = Arc::new(Answered {
      program,
      script: Mutex::new(Box::new(|| 0)),
      checks: Mutex::new(0),
    });
    Self {
      dir,
      _tools: tools,
      db,
      pipelines,
      answered,
    }
  }

  pub fn root(&self) -> &Path {
    self.dir.path()
  }

  pub fn programs(&self) -> Programs {
    Programs::with_runner(self.answered.clone())
  }

  /// The check runs `script` from now on.
  pub fn check_with(&self, script: impl FnMut() -> i32 + Send + 'static) {
    *self.answered.script.lock() = Box::new(script);
  }

  pub fn checks(&self) -> u32 {
    *self.answered.checks.lock()
  }

  pub fn task(&mut self, title: &str) -> Task {
    let details = TaskDetails {
      title: name(title),
      brief: format!("{title}, as the brief says."),
      acceptance_criteria: Vec::new(),
      pipeline: None,
      limits: TaskLimits::NONE,
    };
    self
      .db
      .create_tasks(IdempotencyKey::generate(), vec![details], at(5))
      .expect("task")
      .remove(0)
  }

  /// A run of `task` on `pipeline` from `main`'s head on `branch`, in a worktree of its own, that committed `text` to
  /// `file` and passed both steps at `finished`.
  pub fn succeeded(&mut self, (task, pipeline): (&Task, &str), branch: &str, edit: (&str, &str), finished: i64) -> Run {
    let mut run = self.in_worktree((task, pipeline), branch, finished);
    let worktree = self.worktree(&run);
    fs::write(worktree.join(edit.0), edit.1).expect("edit");
    git(&worktree, &["commit", "-am", &format!("{}: Implement", task.key())]);
    self.pass_both(&mut run, finished);
    run
  }

  /// [`Self::succeeded`] for a run that committed nothing: its branch is where `main` was when it started (ORC-93).
  pub fn succeeded_without_commits(&mut self, (task, pipeline): (&Task, &str), branch: &str, finished: i64) -> Run {
    let mut run = self.in_worktree((task, pipeline), branch, finished);
    self.pass_both(&mut run, finished);
    run
  }

  /// A run of `task` on `pipeline` from `main`'s head on `branch`, started before `finished` in a worktree of its own.
  fn in_worktree(&mut self, (task, pipeline): (&Task, &str), branch: &str, finished: i64) -> Run {
    let base = git(self.root(), &["rev-parse", "main"]);
    let mut run = self
      .db
      .create_run(
        IdempotencyKey::generate(),
        NewRun {
          task: task.id(),
          snapshot: self.pipelines[pipeline].clone(),
          branch: BranchName::new(branch).expect("branch"),
          base: Revision::new(&base).expect("revision"),
          base_branch: Some(BranchName::new("main").expect("branch")),
          budget: None,
          max_duration: None,
        },
        at(finished - 5),
      )
      .expect("run");
    let events = run.start(at(finished - 4)).expect("start");
    self.db.append_events(&run, &events).expect("events");
    let worktree = self.worktree(&run);
    fs::create_dir_all(worktree.parent().expect("parent")).expect("parent");
    git(
      self.root(),
      &["worktree", "add", "-b", branch, worktree.to_str().expect("path")],
    );
    run
  }

  pub fn worktree(&self, run: &Run) -> PathBuf {
    self.root().join(".orchester/worktrees").join(run.id().to_string())
  }

  /// What the runner and the Integrate step's agent do with `run`'s conflicts: its work goes back to Implement, which
  /// merges `main` into the run's branch keeping `text` in `file`, and both steps pass again at `finished`.
  pub fn integrated(&mut self, run: &mut Run, (file, text): (&str, &str), finished: i64) {
    let note = ReviewNote::new("Merge main and resolve the conflicts.").expect("note");
    let implement = StepKey::new("implement").expect("key");
    let events = run
      .return_ended(&implement, (&note, None), Answerer::Automation, at(finished - 3))
      .expect("returned");
    self.db.append_events(run, &events).expect("events");
    let worktree = self.worktree(run);
    // The merge conflicts: its status is not asked for.
    Command::new("git")
      .args(["merge", "main"])
      .current_dir(&worktree)
      .output()
      .expect("git");
    fs::write(worktree.join(file), text).expect("resolve");
    git(&worktree, &["add", "-A"]);
    git(&worktree, &["commit", "--no-edit", "-m", "Merge main"]);
    self.pass_both(run, finished);
  }

  /// Both steps of `run`'s current attempt start and pass, the last at `finished`.
  fn pass_both(&mut self, run: &mut Run, finished: i64) {
    for (index, time) in [(0, finished - 2), (1, finished)] {
      let execution = run.current_attempt().expect("attempt").executions()[index].id();
      let events = run.start_step(execution, at(time - 1)).expect("start step");
      self.db.append_events(run, &events).expect("events");
      let events = run.complete_step(execution, at(time)).expect("pass");
      self.db.append_events(run, &events).expect("events");
    }
  }

  /// Commits `text` to `file` on `main` in the checkout, as a person does; the new head.
  pub fn commit_on_main(root: &Path, (file, text): (&str, &str)) -> String {
    fs::write(root.join(file), text).expect("edit");
    git(root, &["commit", "-am", "A change on main"]);
    git(root, &["rev-parse", "main"])
  }

  /// Moves the queue on until it has nothing to do; the commands it gave for the runner.
  pub fn drive(&self) -> Vec<RunnerCommand> {
    let mut commands = Vec::new();
    for _ in 0..60 {
      match merge_queue::advance(self.root(), &self.programs()).expect("advanced") {
        Advance::Idle { .. } => return commands,
        Advance::Changed => {}
        Advance::Integrate { command, .. } => commands.push(command),
      }
    }
    panic!("the queue never came to rest");
  }

  pub fn state(&self, run: &Run) -> QueueState {
    ProjectDb::open(self.root())
      .expect("open")
      .merge_queue_entry(run.id())
      .expect("entry")
      .expect("in the queue")
      .state
  }

  pub fn events(&self, run: &Run) -> Vec<QueueEvent> {
    ProjectDb::open(self.root())
      .expect("open")
      .merge_queue_events(run.id())
      .expect("events")
      .into_iter()
      .map(|recorded| recorded.event)
      .collect()
  }

  /// Whether `main` holds what `branch` was at.
  pub fn main_holds(&self, commit: &str) -> bool {
    Command::new("git")
      .args(["merge-base", "--is-ancestor", commit, "main"])
      .current_dir(self.root())
      .status()
      .expect("git")
      .success()
  }
}

fn builder(
  db: &mut ProjectDb,
) -> (
  ResourceId,
  BTreeMap<ResourceId, orchester_core::resource::Version<AgentProfile>>,
) {
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  ));
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, at(1))
    .expect("profile")
    .id();
  let version = db.publish_draft::<AgentProfile>(builder, at(1)).expect("publish");
  (builder, BTreeMap::from([(builder, version)]))
}

/// The ways a pipeline delivers, by name; `unfit`'s integrator works in none of its steps, so its conflicts go
/// straight to a person.
fn ways(builder: ResourceId) -> [(&'static str, Option<DeliverStep>); 5] {
  let deliver = |mode, method, integrator| Some(DeliverStep::new(mode, method, integrator, true));
  [
    (
      "approve",
      deliver(DeliverMode::ApproveToMerge, MergeMethod::MergeCommit, Some(builder)),
    ),
    (
      "auto",
      deliver(DeliverMode::AutoMerge, MergeMethod::MergeCommit, Some(builder)),
    ),
    ("squash", deliver(DeliverMode::AutoMerge, MergeMethod::Squash, None)),
    (
      "unfit",
      deliver(
        DeliverMode::AutoMerge,
        MergeMethod::MergeCommit,
        Some(ResourceId::generate()),
      ),
    ),
    ("none", None),
  ]
}

/// The pipelines Implement (Builder, writes) → Check (the check tool), by how they deliver.
fn pipelines(db: &mut ProjectDb, program: &Path) -> BTreeMap<&'static str, RunSnapshot> {
  let (builder, profiles) = builder(db);
  let action = CommandAction::new(
    CommandProgram::new(program.to_str().expect("path")).expect("program"),
    Vec::new(),
  );
  let tool = db
    .create_resource(
      Slug::new("run-tests").expect("slug"),
      name("Run tests"),
      &Tool::command("", action),
      at(1),
    )
    .expect("tool");
  let tools = BTreeMap::from([(tool.id(), db.publish_draft::<Tool>(tool.id(), at(1)).expect("publish"))]);
  let implement = Step {
    key: StepKey::new("implement").expect("key"),
    name: name("Implement"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let check = Step {
    key: StepKey::new("check").expect("key"),
    name: name("Check"),
    kind: StepKind::Tool { tool: Some(tool.id()) },
    depends_on: BTreeSet::from([implement.key.clone()]),
    ..implement.clone()
  };
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &tools,
    instructions: &BTreeMap::new(),
    skills: &BTreeMap::new(),
  };
  ways(builder)
    .into_iter()
    .map(|(way, deliver)| {
      let pipeline = Pipeline::new(vec![implement.clone(), check.clone()])
        .expect("pipeline")
        .with_deliver(deliver);
      let resource = db
        .create_resource(Slug::new(way).expect("slug"), name(way), &pipeline, at(2))
        .expect("pipeline");
      let version = db.publish_draft::<Pipeline>(resource.id(), at(2)).expect("publish");
      (
        way,
        RunSnapshot::build(resource.id(), &version, published).expect("snapshot"),
      )
    })
    .collect()
}
