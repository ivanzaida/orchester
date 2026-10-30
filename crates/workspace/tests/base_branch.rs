//! A run's base branch (ORC-81) against a real git repository whose checkout stays on `main` while runs work on
//! `staging`: a run whose base branch was set merged into `staging` beside the checkout, which is neither switched nor
//! touched, and cleared up after; a run that took the checked-out branch still needs `staging` checked out; moves of
//! `staging` told to its runs; and work merged into `staging` outside Orchester read as merged there.
//! `cargo test -p orchester-workspace --test base_branch base_branch`.

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use orchester_core::{
  id::{IdempotencyKey, ProjectId},
  integration::{MergeError, MergeKind, base::RunBaseSource},
  name::Name,
  project::{Project, ProjectKey},
  resource::{CommandAction, CommandArg, CommandProgram, Pipeline, PromptText, Slug, Step, StepKey, StepKind, Tool},
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunSnapshot},
  store::{MergeStore, ResourceStore, RunStore, TaskStore},
  task::{Task, TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::{
  base_moved,
  tasks::{
    delivery::{self, Delivery},
    integration::{IntegrationProblem, merge_in},
  },
};

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

fn branch(value: &str) -> BranchName {
  BranchName::new(value).expect("branch")
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

/// A project in a git repository whose checkout is on `main` with an uncommitted edit of the person's, and whose
/// `staging` branch is one commit ahead of it; a one-step pipeline its runs pin, and where the runs made from now on
/// took `staging` from.
struct Repo {
  dir: tempfile::TempDir,
  db: ProjectDb,
  snapshot: RunSnapshot,
  source: RunBaseSource,
}

impl Repo {
  fn new() -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-81-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    let dir = tempfile::Builder::new()
      .prefix("base-branch-")
      .tempdir_in(scratch)
      .expect("repo");
    let root = dir.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.name", "Base branch test"]);
    git(root, &["config", "user.email", "test@example.invalid"]);
    git(root, &["config", "core.autocrlf", "false"]);
    fs::write(root.join(".gitignore"), ".orchester/\n").expect("ignore");
    fs::write(root.join("shared.txt"), "base\n").expect("shared");
    fs::write(root.join("notes.txt"), "the person's notes\n").expect("notes");
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "base"]);
    git(root, &["switch", "-c", "staging"]);
    fs::write(root.join("staging.txt"), "staging\n").expect("staging");
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "staging work"]);
    git(root, &["switch", "main"]);
    fs::write(root.join("notes.txt"), "edited, not committed\n").expect("edit");
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("ORC").expect("key"),
      name("Kontur"),
    );
    let mut db = ProjectDb::create(root, project).expect("project");
    let snapshot = snapshot(&mut db);
    Self {
      dir,
      db,
      snapshot,
      source: RunBaseSource::CheckedOut,
    }
  }

  fn root(&self) -> &Path {
    self.dir.path()
  }

  fn task(&mut self, title: &str) -> Task {
    let details = TaskDetails {
      title: name(title),
      brief: String::new(),
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

  /// A run of `task` started from `staging`'s tip on `run_branch` in a worktree of its own, recording `staging` as its
  /// base, whose step is at work after it committed `text` to `file`.
  fn working(&mut self, task: &Task, run_branch: &str, (file, text): (&str, &str)) -> Run {
    let base = git(self.root(), &["rev-parse", "staging"]);
    let mut run = self
      .db
      .create_run(
        IdempotencyKey::generate(),
        NewRun {
          task: task.id(),
          snapshot: self.snapshot.clone(),
          branch: branch(run_branch),
          base: Revision::new(&base).expect("revision"),
          base_branch: Some(branch("staging")),
          base_source: self.source,
          budget: None,
          max_duration: None,
        },
        at(6),
      )
      .expect("run");
    let events = run.start(at(7)).expect("start");
    self.db.append_events(&run, &events).expect("events");
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let events = run.start_step(execution, at(8)).expect("step");
    self.db.append_events(&run, &events).expect("events");
    let worktree = self.worktree(&run);
    fs::create_dir_all(worktree.parent().expect("parent")).expect("parent");
    git(
      self.root(),
      &[
        "worktree",
        "add",
        "-b",
        run_branch,
        worktree.to_str().expect("path"),
        "staging",
      ],
    );
    fs::write(worktree.join(file), text).expect("edit");
    git(&worktree, &["add", "."]);
    git(&worktree, &["commit", "-m", &format!("{}: Implement", task.key())]);
    run
  }

  /// [`Self::working`], whose step then passed: the run is ready to merge.
  fn succeeded(&mut self, task: &Task, run_branch: &str, edit: (&str, &str)) -> Run {
    let mut run = self.working(task, run_branch, edit);
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let events = run.complete_step(execution, at(9)).expect("pass");
    self.db.append_events(&run, &events).expect("events");
    run
  }

  fn worktree(&self, run: &Run) -> PathBuf {
    self.root().join(".orchester/worktrees").join(run.id().to_string())
  }

  /// The checkout is on `main` at `head` with the person's edit and none of the runs' files.
  fn assert_checkout_untouched(&self, head: &str) {
    assert_eq!(git(self.root(), &["symbolic-ref", "--short", "HEAD"]), "main");
    assert_eq!(git(self.root(), &["rev-parse", "HEAD"]), head);
    assert_eq!(git(self.root(), &["status", "--porcelain"]), "M notes.txt");
    assert!(!self.root().join("feature.txt").exists());
  }
}

/// A pipeline of one command step, as runs pin it; what it runs does not matter here.
fn snapshot(db: &mut ProjectDb) -> RunSnapshot {
  let action = CommandAction::new(
    CommandProgram::new("git").expect("program"),
    vec![CommandArg::new("status").expect("arg")],
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

#[test]
fn base_branch_merge_lands_beside_a_checkout_on_another_branch_and_clears_up() {
  let mut repo = Repo::new();
  let following = repo.task("Sign up");
  let checked_out = repo.succeeded(&following, "orchester/orc-1-run-1", ("feature.txt", "work\n"));
  let main = git(repo.root(), &["rev-parse", "main"]);
  let programs = Programs::processes();
  let refused = merge_in(repo.root(), &programs, (checked_out.id(), None)).map(|merged| merged.merge);
  assert!(
    matches!(&refused, Err(IntegrationProblem::Merge(MergeError::WrongBranch { .. }))),
    "a run that took the checked-out branch merges only there: {refused:?}"
  );

  repo.source = RunBaseSource::Task;
  let task = repo.task("Sign in");
  let run = repo.succeeded(&task, "orchester/orc-2-run-1", ("feature.txt", "work\n"));
  let merged = merge_in(repo.root(), &programs, (run.id(), None)).expect("merged beside the checkout");
  assert_eq!(merged.merge.into, branch("staging"));
  assert_eq!(merged.merge.kind, MergeKind::FastForward);
  assert_eq!(
    git(repo.root(), &["show", "staging:feature.txt"]),
    "work",
    "staging holds the run's work"
  );
  repo.assert_checkout_untouched(&main);
  let cleanup = merged.cleanup.expect("cleared up");
  assert!(cleanup.removed_run_worktree(), "the run's worktree went");
  assert!(!repo.worktree(&run).exists());
  assert!(
    git(repo.root(), &["branch", "--list", "orchester/orc-2-run-1"]).is_empty(),
    "its branch, merged into staging, went"
  );
  assert!(
    ProjectDb::open(repo.root())
      .expect("open")
      .run_merge(run.id())
      .expect("read")
      .is_some(),
    "the merge is recorded"
  );
}

#[test]
fn base_branch_moves_of_the_branch_set_are_told_to_its_runs() {
  let mut repo = Repo::new();
  repo.source = RunBaseSource::Task;
  let merged_task = repo.task("Sign in");
  let working_task = repo.task("Sign out");
  let merged = repo.succeeded(&merged_task, "orchester/orc-1-run-1", ("shared.txt", "sign in\n"));
  let working = repo.working(&working_task, "orchester/orc-2-run-1", ("shared.txt", "sign out\n"));
  let programs = Programs::processes();
  assert_eq!(base_moved::observe_in(repo.root(), &programs).expect("observed"), 0);
  merge_in(repo.root(), &programs, (merged.id(), None)).expect("merged beside the checkout");
  let notices = ProjectDb::open(repo.root())
    .expect("open")
    .run_base_notices(working.id())
    .expect("notices");
  assert_eq!(notices.len(), 1, "{notices:?}");
  assert_eq!(notices[0].branch, branch("staging"));
  assert_eq!(
    notices[0].revision.as_str(),
    git(repo.root(), &["rev-parse", "staging"])
  );
  assert_eq!(notices[0].overlapping, ["shared.txt"]);
}

#[test]
fn base_branch_work_merged_into_the_runs_base_outside_orchester_reads_merged_there() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded(&task, "orchester/orc-1-run-1", ("feature.txt", "work\n"));
  let programs = Programs::processes();
  let stored = |repo: &Repo| {
    ProjectDb::open(repo.root())
      .expect("open")
      .run(run.id())
      .expect("read")
      .expect("run")
  };
  assert_eq!(
    delivery::of_run(&programs, repo.root(), &stored(&repo)),
    Delivery::Waiting
  );
  git(repo.root(), &["branch", "-f", "staging", "orchester/orc-1-run-1"]);
  assert_eq!(
    delivery::of_run(&programs, repo.root(), &stored(&repo)),
    Delivery::MergedOutside("staging".into()),
    "staging holds the work while the checkout stays on main"
  );
}
