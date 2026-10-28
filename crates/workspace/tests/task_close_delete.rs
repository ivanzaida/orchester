//! Closing and deleting a task (ORC-91) against a real git repository: a task whose run failed is set Canceled or Done
//! and reopened; one whose run has not ended is closed only once its run was confirmed stopped; a deleted task goes
//! with its runs, conversation, memory records and links, its worktrees and merged branches removed and its unmerged
//! work kept unless the person chose to remove it; a task whose run has not ended and a shared project's task are not
//! deleted. Over MCP, update_task closes a task as the app does, and no tool deletes one. A task whose run ended moved
//! to Todo (the board's Move to Todo, or update_task) changes its status only and starts nothing (ORC-144).
//!
//! `cargo test -p orchester-workspace --test task_close_delete task_`

#[path = "task_close_delete/over_mcp.rs"]
mod over_mcp;

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::{Path, PathBuf},
  process::Command,
  sync::Arc,
};

use orchester_core::{
  conversation::{Message as ConversationMessage, MessageBody, Recipient},
  id::{IdempotencyKey, ProjectId, ResourceId, ServerId, TaskId},
  memory::{
    AgentProvenance, AgentWrite, Confidence, Memory, MemoryContent, MemoryFilter, MemoryKind, MemoryScope,
    MemorySettings, MemoryStore, ScopeFilter, SecretScreen,
  },
  name::Name,
  project::{Project, ProjectKey},
  resource::{CommandAction, CommandArg, CommandProgram, Pipeline, PromptText, Slug, Step, StepKey, StepKind, Tool},
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunSnapshot, RunStatus, StepFailure},
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  sharing::MemberRole,
  store::{ConversationStore, ResourceStore, RunStore, TaskStore},
  task::{
    Task, TaskDetails, TaskLimits,
    organization::{BlockingTask, ChangeActor, DependencyGate, Priority, TaskOrganization, WorkflowStatus},
  },
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb, SharingLink, StoreError};
use orchester_workspace::{
  activity::Activity,
  runner::Runners,
  secrets::Vault,
  sharing::Servers,
  tasks::{
    cancel_run::{Stop, StopRefused},
    close::{Closing, close_in, reopen_in},
    delete::{delete_in, preview_in},
  },
  workspace::{OpenProject, Workspace},
};
use tempfile::TempDir;
use tokio::runtime::Runtime;

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

/// No secret is read or kept here.
struct NoVault;

impl SecretVault for NoVault {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Err(VaultError::Failed("no secrets in these tests".to_owned()))
  }

  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::NotFound)
  }

  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Ok(())
  }
}

/// A project KON in a git repository on `main`, registered in a workspace, with a one-step pipeline its runs pin.
struct Repo {
  dir: TempDir,
  db: ProjectDb,
  snapshot: RunSnapshot,
  runs: usize,
  workspace: Workspace,
  runtime: Runtime,
  _data: TempDir,
}

impl Repo {
  fn new() -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-91-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    let dir = tempfile::Builder::new()
      .prefix("task-close-delete-")
      .tempdir_in(&scratch)
      .expect("repo");
    let root = dir.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.name", "Task deletion test"]);
    git(root, &["config", "user.email", "test@example.invalid"]);
    git(root, &["config", "core.autocrlf", "false"]);
    fs::write(root.join(".gitignore"), ".orchester/\n").expect("ignore");
    fs::write(root.join("README.md"), "Kontur\n").expect("readme");
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "base"]);
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("KON").expect("key"),
      name("Kontur"),
    );
    let mut db = ProjectDb::create(root, project).expect("project");
    let snapshot = snapshot(&mut db);
    let data = tempfile::Builder::new()
      .prefix("task-close-delete-data-")
      .tempdir_in(&scratch)
      .expect("application data");
    let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
    workspace.open(root).expect("register the project");
    Self {
      dir,
      db,
      snapshot,
      runs: 0,
      workspace,
      runtime: Runtime::new().expect("runtime"),
      _data: data,
    }
  }

  fn root(&self) -> &Path {
    self.dir.path()
  }

  fn open(&self) -> OpenProject {
    OpenProject {
      project: self.db.project().clone(),
      repo_root: self.root().to_path_buf(),
    }
  }

  fn servers(&self) -> Servers {
    Servers::new(self.workspace.clone(), Vault::with(Arc::new(NoVault)))
  }

  /// Runners of a session without a runner: nothing here asks one.
  fn runners(&self) -> Runners {
    Runners::new(
      self.root().join("no-runner"),
      self.runtime.handle().clone(),
      Activity::new(),
      orchester_ipc::SessionEnd::Clean,
    )
  }

  /// A draft with every detail, as a task with runs has.
  fn task(&mut self, title: &str) -> Task {
    let details = TaskDetails {
      title: name(title),
      brief: "Sign in with a password.".to_owned(),
      acceptance_criteria: vec!["A person can sign in".to_owned()],
      pipeline: Some(self.snapshot.pipeline().resource),
      limits: TaskLimits::NONE,
    };
    self
      .db
      .create_tasks(IdempotencyKey::generate(), vec![details], at(5))
      .expect("task")
      .remove(0)
  }

  /// A run of `task` from `main` in a worktree of its own, its step at work after it committed a file; with `edit`,
  /// the worktree also holds a change not committed.
  fn working(&mut self, task: &Task, edit: bool) -> Run {
    self.runs += 1;
    let branch = format!("orchester/kon-run-{}", self.runs);
    let base = git(self.root(), &["rev-parse", "main"]);
    let mut run = self
      .db
      .create_run(
        IdempotencyKey::generate(),
        NewRun {
          task: task.id(),
          snapshot: self.snapshot.clone(),
          branch: BranchName::new(&branch).expect("branch"),
          base: Revision::new(&base).expect("revision"),
          base_branch: Some(BranchName::new("main").expect("branch")),
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
    let path = worktree.to_str().expect("path");
    git(self.root(), &["worktree", "add", "-b", &branch, path, "main"]);
    fs::write(worktree.join("feature.txt"), "feature\n").expect("feature");
    git(&worktree, &["add", "."]);
    git(&worktree, &["commit", "-m", "Implement"]);
    if edit {
      fs::write(worktree.join("draft.txt"), "not committed\n").expect("draft");
    }
    run
  }

  /// [`Self::working`], whose step then failed: the run failed.
  fn failed(&mut self, task: &Task, edit: bool) -> Run {
    let mut run = self.working(task, edit);
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let events = run
      .fail_step(execution, StepFailure::ReportedFailed, at(9))
      .expect("fail");
    self.db.append_events(&run, &events).expect("events");
    assert!(matches!(run.status(), RunStatus::Failed(_)), "{:?}", run.status());
    run
  }

  /// [`Self::working`], whose step then passed: the run succeeded with its commit to merge.
  fn succeeded(&mut self, task: &Task) -> Run {
    let mut run = self.working(task, false);
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let events = run.complete_step(execution, at(9)).expect("pass");
    self.db.append_events(&run, &events).expect("events");
    assert_eq!(run.status(), &RunStatus::Succeeded);
    run
  }

  fn worktree(&self, run: &Run) -> PathBuf {
    self.root().join(".orchester/worktrees").join(run.id().to_string())
  }

  fn status(&self, task: TaskId) -> WorkflowStatus {
    let db = ProjectDb::open(self.root()).expect("open");
    db.task_organization(task).expect("organization").status()
  }

  fn close(&self, task: TaskId, closing: Closing, stop: Stop) -> Result<(), StopRefused> {
    self.close_by(task, (closing, stop), ChangeActor::Person)
  }

  fn close_by(&self, task: TaskId, (closing, stop): (Closing, Stop), actor: ChangeActor) -> Result<(), StopRefused> {
    let (open, runners, servers) = (self.open(), self.runners(), self.servers());
    self
      .runtime
      .block_on(close_in(&open, (&runners, &servers), task, (closing, stop, actor)))
  }

  fn last_actor(&self, task: TaskId) -> ChangeActor {
    let history = ProjectDb::open(self.root())
      .expect("open")
      .task_organization_history(task)
      .expect("history");
    history.last().expect("a change").actor
  }

  fn branch_exists(&self, run: &Run) -> bool {
    !git(self.root(), &["branch", "--list", run.branch().as_str()]).is_empty()
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

/// What the task and its run have besides the run: a task memory record, a project record the run's agent wrote, a
/// person's message, and a task it blocks.
fn records(repo: &mut Repo, task: &Task, run: &Run) -> TaskId {
  let screened = |text: &str| {
    SecretScreen::new(Vec::new())
      .check(MemoryContent::new(MemoryKind::Fact, text).expect("content"))
      .expect("no secrets")
  };
  let own = Memory::by_person(MemoryScope::Task(task.id()), screened("Sign-in uses email"), at(10));
  let write = AgentWrite {
    provenance: AgentProvenance {
      profile: ResourceId::generate(),
      run: run.id(),
      step: StepKey::new("check").expect("key"),
      execution: run.current_attempt().expect("attempt").executions()[0].id(),
    },
    confidence: Confidence::Medium,
    sources: Vec::new(),
  };
  let project = Memory::by_agent(
    MemoryScope::Project,
    write,
    screened("The build runs with cargo"),
    &MemorySettings::default(),
    at(11),
  );
  repo.db.add_memories(&[own, project]).expect("memories");
  let message = ConversationMessage::from_person(
    task.id(),
    Some(run.id()),
    Recipient::Orchestrator,
    MessageBody::new("Keep email sign-in").expect("text"),
    at(12),
  );
  repo
    .db
    .send_message(IdempotencyKey::generate(), &message)
    .expect("message");
  let blocked = repo.task("Add sign-out");
  let link = BlockingTask {
    task: task.id(),
    gate: DependencyGate::Merge,
  };
  let organization =
    TaskOrganization::new(WorkflowStatus::Todo, Priority::None, None, vec![link], vec![]).expect("organization");
  repo
    .db
    .set_task_organization(blocked.id(), &organization, ChangeActor::Person, at(13))
    .expect("linked");
  blocked.id()
}

#[test]
fn task_close_sets_canceled_or_done_after_a_failed_run_and_reopens() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  repo.failed(&task, false);

  repo
    .close(task.id(), Closing::Canceled, Stop::Unconfirmed)
    .expect("a failed run stops nothing");
  assert_eq!(repo.status(task.id()), WorkflowStatus::Canceled);
  assert_eq!(repo.last_actor(task.id()), ChangeActor::Person);

  reopen_in(repo.root(), &repo.servers(), task.id(), ChangeActor::Person).expect("reopened");
  assert_eq!(
    repo.status(task.id()),
    WorkflowStatus::Backlog,
    "a draft reopens to Backlog"
  );

  repo
    .close(task.id(), Closing::Done, Stop::Unconfirmed)
    .expect("closed as Done");
  assert_eq!(repo.status(task.id()), WorkflowStatus::Done);

  // A press from an MCP client's input is automation's, as update_task's changes are.
  reopen_in(repo.root(), &repo.servers(), task.id(), ChangeActor::Orchester).expect("reopened");
  assert_eq!(repo.last_actor(task.id()), ChangeActor::Orchester);
  let closed = repo.close_by(
    task.id(),
    (Closing::Canceled, Stop::Unconfirmed),
    ChangeActor::Orchester,
  );
  assert_eq!(closed, Ok(()));
  assert_eq!(repo.last_actor(task.id()), ChangeActor::Orchester);
}

#[test]
fn task_close_asks_to_stop_a_run_that_has_not_ended() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.working(&task, false);

  for closing in [Closing::Done, Closing::Canceled] {
    let refused = repo.close(task.id(), closing, Stop::Unconfirmed);
    assert_eq!(refused, Err(StopRefused::Unconfirmed(run.id())), "{closing:?}");
    // The store checks again as it writes: a run that started after the stop is named, not left running.
    let mut db = ProjectDb::open(repo.root()).expect("open");
    let written = db.close_task_organization(task.id(), closing.status(), (ChangeActor::Person, at(20)));
    assert!(
      matches!(written, Err(StoreError::Organization(_))),
      "{closing:?}: {written:?}"
    );
  }
  let other = repo.close(task.id(), Closing::Canceled, Stop::Confirmed(None));
  assert_eq!(other, Err(StopRefused::StartedSince(run.id())));
  assert_eq!(repo.status(task.id()), WorkflowStatus::InProgress, "nothing changed");
}

#[test]
fn task_delete_removes_the_task_with_its_runs_and_records_and_keeps_unmerged_work() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.failed(&task, true);
  let blocked = records(&mut repo, &task, &run);
  let programs = Programs::processes();

  let preview = preview_in(repo.root(), &programs, task.id()).expect("deletable");
  assert_eq!(
    (
      preview.facts.runs,
      preview.facts.memories,
      preview.facts.project_memories
    ),
    (1, 1, 1)
  );
  assert_eq!(preview.facts.blocking, 1);
  assert_eq!((preview.changed_worktrees, preview.unmerged_branches), (1, 1));
  assert!(preview.has_unmerged_work());

  delete_in(repo.root(), &programs, task.id(), false).expect("deleted");
  let db = ProjectDb::open(repo.root()).expect("open");
  assert!(db.task(task.id()).expect("read").is_none(), "the task is gone");
  assert!(db.run(run.id()).expect("read").is_none(), "its run is gone");
  assert!(
    db.messages(task.id()).expect("read").is_empty(),
    "its conversation is gone"
  );
  let every = MemoryFilter {
    scope: ScopeFilter::All,
    status: None,
    pinned_only: false,
  };
  assert!(
    db.memories(&every).expect("read").is_empty(),
    "its memory records are gone"
  );
  assert!(
    db.task_organization(blocked).expect("read").dependencies().is_empty(),
    "the task it blocked is no longer linked"
  );
  assert!(repo.worktree(&run).exists(), "the worktree with uncommitted work stays");
  assert!(repo.branch_exists(&run), "the unmerged branch stays");
}

#[test]
fn task_delete_with_unmerged_work_removes_its_worktrees_and_branches() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.failed(&task, true);
  let main = git(repo.root(), &["rev-parse", "main"]);

  delete_in(repo.root(), &Programs::processes(), task.id(), true).expect("deleted");
  assert!(!repo.worktree(&run).exists(), "the worktree went");
  assert!(!repo.branch_exists(&run), "the unmerged branch went");
  assert_eq!(git(repo.root(), &["rev-parse", "main"]), main, "main is untouched");
}

#[test]
fn task_delete_is_refused_while_a_run_has_not_ended() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.working(&task, false);
  let programs = Programs::processes();

  let refused = preview_in(repo.root(), &programs, task.id()).expect_err("not deletable");
  assert_eq!(refused.key, "tasks.delete.stopRunFirst");
  let refused = delete_in(repo.root(), &programs, task.id(), true).expect_err("not deleted");
  assert_eq!(refused.key, "tasks.delete.stopRunFirst");
  let db = ProjectDb::open(repo.root()).expect("open");
  assert!(db.task(task.id()).expect("read").is_some(), "the task stays");
  assert!(repo.worktree(&run).exists(), "its worktree stays");
}

#[test]
fn task_delete_is_unavailable_in_a_shared_project() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("KON").expect("key"),
    role: Some(MemberRole::Owner),
    joined_at: at(100),
    cursor: 1,
    last_synced_at: at(100),
    removed_at: None,
    address: None,
  };
  repo
    .db
    .start_sharing(&link, &[], (&[], &Default::default()))
    .expect("shared");

  let refused = preview_in(repo.root(), &Programs::processes(), task.id()).expect_err("not deletable");
  assert_eq!(refused.key, "tasks.delete.shared");
  let mut db = ProjectDb::open(repo.root()).expect("open");
  assert!(matches!(
    db.delete_task(task.id(), at(200)),
    Err(StoreError::SharedProject)
  ));
  assert!(db.task(task.id()).expect("read").is_some(), "the task stays");
}
