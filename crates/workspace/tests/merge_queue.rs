//! The merge queue (ORC-19, `orchester_workspace::merge_queue`) against a real git repository: runs of a pipeline
//! that ends in a Deliver step join it and deliver one at a time, first finished first (synced with `main`, checked
//! again on the merged result, merged, cleared); a person moves waiting runs up or to the top; a base that moves during
//! the check means syncing and checking again, at most three times in a row, and a run is never merged on a check of
//! an older base; conflicts go to the Integrate step, and when it fails nothing reaches `main`; a dirty checkout
//! blocks the queue until it is clean; a run waiting for approval does not hold later runs; a pipeline without a
//! Deliver step keeps its runs out of the queue. The check is a command tool whose program the tests answer for.
//! `cargo test -p orchester-workspace --test merge_queue merge_queue`.

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
  merge_queue::{Blocked, Left, QueueEvent, QueueState},
  name::Name,
  permission::{AgentPermissions, CommandPermission, NetworkAccess, StepPermissions},
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, CommandAction, CommandProgram, DeliverMode, DeliverStep, MergeMethod, ModelId, Pipeline, PromptText,
    ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind, Tool,
  },
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunSnapshot},
  store::{MergeStore, ResourceStore, RunStore, TaskStore},
  task::{Task, TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_exec::{ProcessRunner, programs::Programs};
use orchester_ipc::Command as RunnerCommand;
use orchester_store::ProjectDb;
use orchester_workspace::merge_queue::{self, Advance, Move};
use parking_lot::Mutex;

fn git(root: &Path, args: &[&str]) -> String {
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

fn at(millis: i64) -> Timestamp {
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
struct Repo {
  dir: tempfile::TempDir,
  /// Where the check's program lives, outside the repository.
  _tools: tempfile::TempDir,
  db: ProjectDb,
  pipelines: BTreeMap<&'static str, RunSnapshot>,
  answered: Arc<Answered>,
}

impl Repo {
  fn new() -> Self {
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

  fn root(&self) -> &Path {
    self.dir.path()
  }

  fn programs(&self) -> Programs {
    Programs::with_runner(self.answered.clone())
  }

  /// The check runs `script` from now on.
  fn check_with(&self, script: impl FnMut() -> i32 + Send + 'static) {
    *self.answered.script.lock() = Box::new(script);
  }

  fn checks(&self) -> u32 {
    *self.answered.checks.lock()
  }

  fn task(&mut self, title: &str) -> Task {
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
  fn succeeded(&mut self, (task, pipeline): (&Task, &str), branch: &str, edit: (&str, &str), finished: i64) -> Run {
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
    git(self.root(), &["worktree", "add", "-b", branch, worktree.to_str().expect("path")]);
    fs::write(worktree.join(edit.0), edit.1).expect("edit");
    git(&worktree, &["commit", "-am", &format!("{}: Implement", task.key())]);
    for (index, time) in [(0, finished - 3), (1, finished)] {
      let execution = run.current_attempt().expect("attempt").executions()[index].id();
      let events = run.start_step(execution, at(time - 1)).expect("start step");
      self.db.append_events(&run, &events).expect("events");
      let events = run.complete_step(execution, at(time)).expect("pass");
      self.db.append_events(&run, &events).expect("events");
    }
    run
  }

  fn worktree(&self, run: &Run) -> PathBuf {
    self.root().join(".orchester/worktrees").join(run.id().to_string())
  }

  /// Commits `text` to `file` on `main` in the checkout, as a person does; the new head.
  fn commit_on_main(root: &Path, (file, text): (&str, &str)) -> String {
    fs::write(root.join(file), text).expect("edit");
    git(root, &["commit", "-am", "A change on main"]);
    git(root, &["rev-parse", "main"])
  }

  /// Moves the queue on until it has nothing to do; the commands it gave for the runner.
  fn drive(&self) -> Vec<RunnerCommand> {
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

  fn state(&self, run: &Run) -> QueueState {
    ProjectDb::open(self.root())
      .expect("open")
      .merge_queue_entry(run.id())
      .expect("entry")
      .expect("in the queue")
      .state
  }

  fn events(&self, run: &Run) -> Vec<QueueEvent> {
    ProjectDb::open(self.root())
      .expect("open")
      .merge_queue_events(run.id())
      .expect("events")
      .into_iter()
      .map(|recorded| recorded.event)
      .collect()
  }

  /// Whether `main` holds what `branch` was at.
  fn main_holds(&self, commit: &str) -> bool {
    Command::new("git")
      .args(["merge-base", "--is-ancestor", commit, "main"])
      .current_dir(self.root())
      .status()
      .expect("git")
      .success()
  }
}

fn builder(db: &mut ProjectDb) -> (ResourceId, BTreeMap<ResourceId, orchester_core::resource::Version<AgentProfile>>) {
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

/// The pipelines Implement (Builder, writes) → Check (the check tool), by how they deliver.
fn pipelines(db: &mut ProjectDb, program: &Path) -> BTreeMap<&'static str, RunSnapshot> {
  let (builder, profiles) = builder(db);
  let action = CommandAction::new(
    CommandProgram::new(program.to_str().expect("path")).expect("program"),
    Vec::new(),
  );
  let tool = db
    .create_resource(Slug::new("run-tests").expect("slug"), name("Run tests"), &Tool::command("", action), at(1))
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
  let ways = [
    ("approve", Some(DeliverStep::new(DeliverMode::ApproveToMerge, MergeMethod::MergeCommit, Some(builder), true))),
    ("auto", Some(DeliverStep::new(DeliverMode::AutoMerge, MergeMethod::MergeCommit, Some(builder), true))),
    ("squash", Some(DeliverStep::new(DeliverMode::AutoMerge, MergeMethod::Squash, None, true))),
    ("none", None),
  ];
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &tools,
    instructions: &BTreeMap::new(),
    skills: &BTreeMap::new(),
  };
  ways
    .into_iter()
    .map(|(way, deliver)| {
      let pipeline = Pipeline::new(vec![implement.clone(), check.clone()])
        .expect("pipeline")
        .with_deliver(deliver);
      let resource = db
        .create_resource(Slug::new(way).expect("slug"), name(way), &pipeline, at(2))
        .expect("pipeline");
      let version = db.publish_draft::<Pipeline>(resource.id(), at(2)).expect("publish");
      (way, RunSnapshot::build(resource.id(), &version, published).expect("snapshot"))
    })
    .collect()
}

#[test]
fn merge_queue_delivers_one_run_at_a_time_first_finished_first_and_clears_after_it() {
  let mut repo = Repo::new();
  let (first, second) = (repo.task("Sign in"), repo.task("Sign out"));
  let later = repo.succeeded((&second, "auto"), "orchester/orc-2-run-1", ("other.txt", "second\n"), 200);
  let earlier = repo.succeeded((&first, "auto"), "orchester/orc-1-run-2", ("shared.txt", "first\n"), 100);
  let tips = (git(repo.root(), &["rev-parse", "orchester/orc-1-run-2"]), git(repo.root(), &["rev-parse", "orchester/orc-2-run-1"]));

  repo.drive();

  let QueueState::Merged(merged_first) = repo.state(&earlier) else {
    panic!("{:?}", repo.state(&earlier));
  };
  let QueueState::Merged(merged_second) = repo.state(&later) else {
    panic!("{:?}", repo.state(&later));
  };
  assert!(repo.main_holds(&tips.0) && repo.main_holds(&tips.1));
  let order = git(repo.root(), &["log", "--first-parent", "--format=%H", "-2", "main"]);
  let order: Vec<&str> = order.lines().collect();
  assert_eq!(order, [merged_second.revision.as_str(), merged_first.revision.as_str()], "first finished, first merged");
  assert!(merged_first.worktree_removed && merged_first.branch_deleted, "{merged_first:?}");
  assert!(!repo.worktree(&earlier).exists());
  assert_eq!(repo.checks(), 2, "Check ran once on each merged result");
  let db = ProjectDb::open(repo.root()).expect("open");
  assert!(db.run_merge(earlier.id()).expect("merge").is_some(), "the merge is recorded, so the task is done");
  let events = repo.events(&later);
  assert!(
    matches!(events.as_slice(), [QueueEvent::Joined, QueueEvent::Synced { files, .. }, QueueEvent::CheckStarted { .. }, QueueEvent::CheckPassed { .. }, QueueEvent::Merged { .. }, QueueEvent::CleanedUp { .. }] if files.is_empty()),
    "{events:?}"
  );
}

#[test]
fn merge_queue_squashes_into_one_commit_titled_by_the_task_and_deletes_the_branch() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "squash"), "orchester/orc-1-run-1", ("shared.txt", "run\n"), 100);
  repo.drive();
  let QueueState::Merged(merged) = repo.state(&run) else {
    panic!("{:?}", repo.state(&run));
  };
  assert_eq!(git(repo.root(), &["log", "-1", "--format=%s", "main"]), format!("{}: Sign in", task.key()));
  assert_eq!(git(repo.root(), &["rev-list", "--count", "main"]), "2", "one commit on top of the base");
  assert!(merged.branch_deleted, "{merged:?}");
  assert!(git(repo.root(), &["branch", "--list", "orchester/orc-1-run-1"]).is_empty());
}

#[test]
fn merge_queue_moves_waiting_runs_up_and_to_the_top_and_removes_them() {
  let mut repo = Repo::new();
  let tasks: Vec<Task> = ["One", "Two", "Three"].into_iter().map(|title| repo.task(title)).collect();
  let runs: Vec<Run> = tasks
    .iter()
    .enumerate()
    .map(|(index, task)| {
      let branch = format!("orchester/orc-{}-run-1", index + 1);
      let time = 100 * (i64::try_from(index).expect("small") + 1);
      repo.succeeded((task, "approve"), &branch, ("other.txt", "edit\n"), time)
    })
    .collect();
  let mut db = ProjectDb::open(repo.root()).expect("open");
  assert!(merge_queue::join(&mut db, at(1_000)).expect("joined"));
  let order = |repo: &Repo| -> Vec<u32> {
    merge_queue::read(repo.root(), &repo.programs())
      .expect("read")
      .rows
      .iter()
      .map(|row| row.run_number)
      .collect()
  };
  let numbers: Vec<u32> = runs.iter().map(|run| run.number().get()).collect();
  assert_eq!(order(&repo), numbers, "first finished first");
  assert_eq!(
    merge_queue::move_run(repo.root(), runs[0].id(), Move::Up),
    Err(merge_queue::QueueRefusal::First)
  );
  merge_queue::move_run(repo.root(), runs[2].id(), Move::Up).expect("moved up");
  assert_eq!(order(&repo), [numbers[0], numbers[2], numbers[1]]);
  merge_queue::move_run(repo.root(), runs[1].id(), Move::ToTop).expect("moved to the top");
  assert_eq!(order(&repo), [numbers[1], numbers[0], numbers[2]]);
  merge_queue::remove(repo.root(), runs[0].id(), at(1_001)).expect("removed");
  assert_eq!(repo.state(&runs[0]), QueueState::Removed);
  assert_eq!(order(&repo), [numbers[1], numbers[2]], "a removed run is not shown and does not join again");
  let mut db = ProjectDb::open(repo.root()).expect("open");
  assert!(!merge_queue::join(&mut db, at(1_002)).expect("joined"));
}

#[test]
fn merge_queue_syncs_again_when_main_moves_during_the_check_and_never_merges_on_an_older_base() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "auto"), "orchester/orc-1-run-1", ("shared.txt", "run\n"), 100);
  let root = repo.root().to_path_buf();
  let moved = Arc::new(Mutex::new(None));
  let seen = moved.clone();
  repo.check_with(move || {
    let mut moved = seen.lock();
    if moved.is_none() {
      *moved = Some(Repo::commit_on_main(&root, ("other.txt", "moved during the check\n")));
    }
    0
  });
  repo.drive();
  let QueueState::Merged(merged) = repo.state(&run) else {
    panic!("{:?}", repo.state(&run));
  };
  let moved = moved.lock().clone().expect("main moved");
  assert_eq!(repo.checks(), 2, "the check ran again once main moved");
  let events = repo.events(&run);
  assert!(events.iter().any(|event| matches!(event, QueueEvent::CheckSuperseded { now, .. } if now.as_str() == moved)), "{events:?}");
  let passed: Vec<&Revision> = events
    .iter()
    .filter_map(|event| match event {
      QueueEvent::CheckPassed { base, .. } => Some(base),
      _ => None,
    })
    .collect();
  assert_eq!(passed.last().map(|base| base.as_str()), Some(moved.as_str()), "the check that counted ran on main as it was merged into");
  assert_eq!(git(repo.root(), &["rev-parse", &format!("{}^1", merged.revision)]), moved);
}

#[test]
fn merge_queue_takes_a_run_out_once_main_keeps_moving_and_nothing_reaches_main() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "auto"), "orchester/orc-1-run-1", ("shared.txt", "run\n"), 100);
  let tip = git(repo.root(), &["rev-parse", "orchester/orc-1-run-1"]);
  let root = repo.root().to_path_buf();
  let mut count = 0;
  repo.check_with(move || {
    count += 1;
    Repo::commit_on_main(&root, ("other.txt", &format!("move {count}\n")));
    0
  });
  repo.drive();
  let QueueState::Left(Left::KeepsMoving { base, heads }) = repo.state(&run) else {
    panic!("{:?}", repo.state(&run));
  };
  assert_eq!(base, "main");
  assert_eq!(repo.checks(), 4, "three re-syncs in a row, then it leaves");
  assert_eq!(heads.len(), 4, "{heads:?}");
  assert!(!repo.main_holds(&tip), "nothing of the run reached main");
}

#[test]
fn merge_queue_sends_conflicts_to_the_integrate_step_and_when_it_fails_nothing_reaches_main() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "auto"), "orchester/orc-1-run-1", ("shared.txt", "the run's line\n"), 100);
  let tip = git(repo.root(), &["rev-parse", "orchester/orc-1-run-1"]);
  Repo::commit_on_main(repo.root(), ("shared.txt", "main's line\n"));
  let commands = repo.drive();
  let [RunnerCommand::ReturnRun(returned)] = commands.as_slice() else {
    panic!("{commands:?}");
  };
  assert_eq!(returned.run, run.id());
  assert_eq!(returned.step.as_str(), "implement", "the step the integrator profile works in");
  assert_eq!(returned.conflicts.as_ref().map(|conflicts| conflicts.files.clone()), Some(vec!["shared.txt".to_owned()]));
  assert!(matches!(repo.state(&run), QueueState::Integrating { .. }), "{:?}", repo.state(&run));
  assert_eq!(git(&repo.worktree(&run), &["status", "--porcelain"]), "", "the sync that conflicted was put back");

  let refused = orchester_workspace::messages::Message::new("errors.storage");
  merge_queue::integrate_sent(repo.root(), run.id(), Err(refused)).expect("told");
  assert_eq!(
    repo.state(&run),
    QueueState::Left(Left::Conflicts {
      files: vec!["shared.txt".to_owned()]
    })
  );
  assert!(!repo.main_holds(&tip), "nothing of the run reached main");
  assert_eq!(repo.drive(), Vec::new(), "the run left the queue");
}

#[test]
fn merge_queue_blocks_on_a_dirty_checkout_and_goes_on_once_it_is_clean() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "auto"), "orchester/orc-1-run-1", ("shared.txt", "run\n"), 100);
  fs::write(repo.root().join("other.txt"), "not committed\n").expect("edit");
  repo.drive();
  assert_eq!(
    repo.state(&run),
    QueueState::Blocked(Blocked::Uncommitted {
      files: vec!["M other.txt".to_owned()]
    })
  );
  assert_eq!(
    merge_queue::advance(repo.root(), &repo.programs()).expect("advanced"),
    Advance::Idle { poll: true },
    "it looks again later, by itself"
  );
  git(repo.root(), &["checkout", "--", "other.txt"]);
  repo.drive();
  assert!(matches!(repo.state(&run), QueueState::Merged(_)), "{:?}", repo.state(&run));
}

#[test]
fn merge_queue_waiting_for_approval_does_not_hold_later_runs_and_checks_again_once_approved() {
  let mut repo = Repo::new();
  let (first, second) = (repo.task("Sign in"), repo.task("Sign out"));
  let waiting = repo.succeeded((&first, "approve"), "orchester/orc-1-run-1", ("shared.txt", "first\n"), 100);
  let later = repo.succeeded((&second, "auto"), "orchester/orc-2-run-1", ("other.txt", "second\n"), 200);
  repo.drive();
  assert_eq!(repo.state(&waiting), QueueState::WaitingForApproval);
  assert!(matches!(repo.state(&later), QueueState::Merged(_)), "{:?}", repo.state(&later));
  let read = merge_queue::read(repo.root(), &repo.programs()).expect("read");
  let row = read.rows.iter().find(|row| row.entry.run == waiting.id()).expect("row");
  assert!(row.base_moved, "main moved since its check");

  merge_queue::approve(repo.root(), waiting.id(), at(1_000)).expect("approved");
  repo.drive();
  assert!(matches!(repo.state(&waiting), QueueState::Merged(_)), "{:?}", repo.state(&waiting));
  assert_eq!(repo.checks(), 3, "the approved run was checked again on main as it was then");
}

#[test]
fn merge_queue_leaves_out_runs_whose_pipeline_has_no_deliver_step_and_takes_a_failed_check_out() {
  let mut repo = Repo::new();
  let (manual, checked) = (repo.task("Merged by hand"), repo.task("Fails its check"));
  let by_hand = repo.succeeded((&manual, "none"), "orchester/orc-1-run-1", ("other.txt", "by hand\n"), 100);
  let failing = repo.succeeded((&checked, "auto"), "orchester/orc-2-run-1", ("shared.txt", "run\n"), 200);
  let tip = git(repo.root(), &["rev-parse", "orchester/orc-2-run-1"]);
  repo.check_with(|| 1);
  repo.drive();
  let db = ProjectDb::open(repo.root()).expect("open");
  assert_eq!(db.merge_queue_entry(by_hand.id()).expect("entry"), None, "it keeps today's way");
  assert!(matches!(repo.state(&failing), QueueState::Left(Left::CheckFailed { .. })), "{:?}", repo.state(&failing));
  assert!(!repo.main_holds(&tip), "nothing of the run reached main");
}
