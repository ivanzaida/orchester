//! Claims end with their runs (ORC-284, `docs/architecture/server.md`, "Claims"), against a real `orchester-server` on
//! loopback behind a team server that takes the run progress report (`sharing_support/run_reports.rs`): a merged run,
//! in Orchester or by a person without it, completes its claim through the report, a cancelled one releases it, and a
//! succeeded or failed one keeps it, on a sync and on a run change. The heartbeat no longer keeps an ended claim fresh,
//! and a server without the report refuses it as not supported until a later sync finds one that has it.
//!
//! `cargo test -p orchester-workspace --test claim_lifecycle claim_lifecycle_`

#[path = "sharing_support/run_reports.rs"]
mod run_reports;
#[path = "server_support/mod.rs"]
mod server_support;
#[path = "sharing_support/project.rs"]
mod support;

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::Path,
  process::Command,
  time::Duration,
};

use orchester_api::{ClaimTake, SharedTask};
use orchester_core::{
  id::{IdempotencyKey, ProjectId, TaskId},
  integration::{MergeKind, RunMerge},
  name::Name,
  project::{Project, ProjectKey},
  resource::{CommandAction, CommandArg, CommandProgram, Pipeline, PromptText, Slug, Step, StepKey, StepKind, Tool},
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunSnapshot, RunStatus, StepFailure, TaskPhase},
  sharing::{ClaimEnd, ClaimKind},
  store::{MergeStore, ResourceStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::sharing::project::{ClaimEndError, WorkClaim};
use run_reports::ReportingServer;
use server_support::TestServer;
use support::{Other, Person, details, member};
use tempfile::TempDir;

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

/// Alice's shared project Kontur in a git repository, its four tasks, and Oleh, a member who reads the server.
struct Team {
  front: ReportingServer,
  alice: Person,
  oleh: Other,
  tasks: [TaskId; 4],
  snapshot: RunSnapshot,
  programs: Programs,
  runs: usize,
  server: TestServer,
}

impl Team {
  /// Alice joined the team server through `front`, which does not take reports until it is told to.
  fn new() -> Self {
    let server = TestServer::start();
    let front = ReportingServer::start(server.address);
    let (repo, tasks, snapshot) = repository();
    let code = server.code_for("alice", true);
    let alice = Person::with_repo_at(&server, &front.url(), ("alice", "Alice Moreau", &code), repo);
    alice.share(&server);
    let oleh = Other::new(&server, "oleh", "Oleh Petrenko");
    member(&alice, "oleh");
    Self {
      front,
      alice,
      oleh,
      tasks,
      snapshot,
      programs: Programs::processes(),
      runs: 0,
      server,
    }
  }

  fn root(&self) -> &Path {
    self.alice.root()
  }

  /// Alice takes the run claim of `task`, as starting its run does.
  fn claimed(&self, task: TaskId) {
    let claim = self.work_on(task);
    assert!(claim.newly_taken());
  }

  /// Work on `task` asks for its run claim, as Run again does before the runner records the new run.
  fn work_on(&self, task: TaskId) -> WorkClaim {
    self
      .alice
      .block_on(self.alice.servers.claim_for_work(self.root(), task))
      .expect("alice holds the task")
  }

  /// Alice's other computer takes the run claim of `task` through the server's API.
  fn claimed_elsewhere(&self, task: TaskId) {
    let project = self.alice.server().link().project;
    let elsewhere = self.server.device_of("alice");
    let take = ClaimTake { kind: ClaimKind::Run };
    self
      .alice
      .block_on(self.oleh.client.claim(&elsewhere, project, task, &take))
      .expect("Alice's other computer takes the task");
  }

  /// A run of `task` from `main`, its one step at work.
  fn working(&mut self, task: TaskId) -> (ProjectDb, Run) {
    self.runs += 1;
    let mut db = self.alice.db();
    let base = git(self.root(), &["rev-parse", "main"]);
    let new = NewRun {
      task,
      snapshot: self.snapshot.clone(),
      branch: BranchName::new(&format!("orchester/orc-run-{}", self.runs)).expect("branch"),
      base: Revision::new(&base).expect("revision"),
      base_branch: Some(BranchName::new("main").expect("branch")),
      budget: None,
      max_duration: None,
    };
    let mut run = db
      .create_run(IdempotencyKey::generate(), new, Timestamp::now())
      .expect("run");
    let events = run.start(Timestamp::now()).expect("start");
    db.append_events(&run, &events).expect("events");
    let events = run.start_step(execution(&run), Timestamp::now()).expect("step");
    db.append_events(&run, &events).expect("events");
    (db, run)
  }

  /// A run of `task` whose step passed: its work waits to be merged.
  fn succeeded(&mut self, task: TaskId) -> Run {
    let (mut db, mut run) = self.working(task);
    let events = run.complete_step(execution(&run), Timestamp::now()).expect("pass");
    db.append_events(&run, &events).expect("events");
    assert_eq!(run.status(), &RunStatus::Succeeded);
    run
  }

  /// A run of `task` that succeeded and that Orchester merged.
  fn merged(&mut self, task: TaskId) {
    let run = self.succeeded(task);
    let merge = RunMerge {
      run: run.id(),
      into: BranchName::new("main").expect("branch"),
      revision: Revision::new(&git(self.root(), &["rev-parse", "main"])).expect("revision"),
      kind: MergeKind::MergeCommit,
      merged_at: Timestamp::now(),
    };
    self.alice.db().save_run_merge(&merge).expect("merged");
  }

  fn failed(&mut self, task: TaskId) {
    let (mut db, mut run) = self.working(task);
    let events = run
      .fail_step(execution(&run), StepFailure::ReportedFailed, Timestamp::now())
      .expect("fail");
    db.append_events(&run, &events).expect("events");
    assert!(matches!(run.status(), RunStatus::Failed(_)), "{:?}", run.status());
  }

  fn cancelled(&mut self, task: TaskId) {
    let (mut db, mut run) = self.working(task);
    let events = run.cancel(Timestamp::now()).expect("cancel");
    db.append_events(&run, &events).expect("events");
    assert_eq!(run.status(), &RunStatus::Cancelled);
  }

  /// A person commits on `run`'s branch and merges it into `main` with git, without Orchester.
  fn merge_outside(&self, run: &Run) {
    let root = self.root();
    git(root, &["checkout", "-b", run.branch().as_str()]);
    fs::write(root.join("feature.txt"), "feature\n").expect("feature");
    git(root, &["add", "feature.txt"]);
    git(root, &["commit", "-m", "Implement"]);
    git(root, &["checkout", "main"]);
    git(
      root,
      &["merge", "--no-ff", "-m", "Merge the run", run.branch().as_str()],
    );
  }

  fn on_server(&self, task: TaskId) -> SharedTask {
    let project = self.alice.server().link().project;
    self
      .alice
      .block_on(self.oleh.client.tasks(&self.oleh.token, project))
      .expect("tasks")
      .tasks
      .into_iter()
      .find(|shared| shared.id == task)
      .expect("the task")
  }

  /// When the server last kept `task`'s claim fresh.
  fn heartbeat_at(&self, task: TaskId) -> i64 {
    self.on_server(task).claim.expect("a claim").heartbeat_at
  }

  /// The claims Alice holds as her cache has them.
  fn held_here(&self) -> BTreeSet<TaskId> {
    let me = self.alice.server().me().expect("signed in");
    self
      .alice
      .db()
      .claims_held_by(me)
      .expect("claims")
      .into_iter()
      .collect()
  }

  fn sync(&self, beat: bool) {
    self
      .alice
      .block_on(self.alice.servers.sync_shared(beat, &self.programs));
  }
}

fn execution(run: &Run) -> orchester_core::id::StepExecutionId {
  run.current_attempt().expect("attempt").executions()[0].id()
}

/// A git repository on `main` with the project Kontur (key ORC) of four tasks, and a pipeline its runs pin.
fn repository() -> (TempDir, [TaskId; 4], RunSnapshot) {
  let repo = TempDir::new().expect("repository");
  let root = repo.path();
  git(root, &["init", "-b", "main"]);
  git(root, &["config", "user.name", "Claim lifecycle test"]);
  git(root, &["config", "user.email", "test@example.invalid"]);
  git(root, &["config", "core.autocrlf", "false"]);
  fs::write(root.join(".gitignore"), ".orchester/\n").expect("ignore");
  fs::write(root.join("README.md"), "Kontur\n").expect("readme");
  git(root, &["add", "."]);
  git(root, &["commit", "-m", "base"]);
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    name("Kontur"),
  );
  let mut db = ProjectDb::create(root, project).expect("project");
  let titles = ["Repository switcher", "Fix reconnect", "Merge queue", "Token usage"];
  let tasks: Vec<TaskId> = db
    .create_tasks(
      IdempotencyKey::generate(),
      titles.iter().map(|title| details(title)).collect(),
      Timestamp::now(),
    )
    .expect("tasks")
    .iter()
    .map(orchester_core::task::Task::id)
    .collect();
  let snapshot = snapshot(&mut db);
  (repo, tasks.try_into().expect("four tasks"), snapshot)
}

/// A pipeline of one command step, as runs pin it; what it runs does not matter here.
fn snapshot(db: &mut ProjectDb) -> RunSnapshot {
  let at = Timestamp::now();
  let action = CommandAction::new(
    CommandProgram::new("git").expect("program"),
    vec![CommandArg::new("status").expect("arg")],
  );
  let tool = db
    .create_resource(
      Slug::new("check-tool").expect("slug"),
      name("Check"),
      &Tool::command("", action),
      at,
    )
    .expect("tool");
  let tool_version = db.publish_draft::<Tool>(tool.id(), at).expect("publish");
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
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, at)
    .expect("pipeline");
  let version = db.publish_draft::<Pipeline>(resource.id(), at).expect("publish");
  let tools = BTreeMap::from([(tool.id(), tool_version)]);
  let published = PublishedVersions {
    profiles: &BTreeMap::new(),
    tools: &tools,
    instructions: &BTreeMap::new(),
    skills: &BTreeMap::new(),
  };
  RunSnapshot::build(resource.id(), &version, published).expect("snapshot")
}

/// The case ORC-281 found: a sync ends the claims of runs that ended, also of runs that ended before it existed.
#[test]
fn claim_lifecycle_a_sync_completes_a_merged_claim_releases_a_cancelled_one_and_keeps_the_others() {
  let mut team = Team::new();
  team.front.take_reports();
  let [merged, cancelled, succeeded, failed] = team.tasks;
  for task in team.tasks {
    team.claimed(task);
  }
  team.merged(merged);
  team.cancelled(cancelled);
  team.succeeded(succeeded);
  team.failed(failed);

  team.sync(false);
  assert_eq!(
    team.front.taken(),
    [(merged.to_string(), TaskPhase::Merged)],
    "only the merged run is reported; a cancelled one's claim is released"
  );
  assert!(team.on_server(merged).claim.is_none(), "completed");
  assert!(team.on_server(cancelled).claim.is_none(), "released");
  assert!(team.on_server(succeeded).claim.is_some(), "kept for review and merge");
  assert!(team.on_server(failed).claim.is_some(), "kept for Run again");
  assert_eq!(team.held_here(), BTreeSet::from([succeeded, failed]));
}

#[test]
fn claim_lifecycle_a_run_change_completes_the_claim_of_work_merged_without_orchester() {
  let mut team = Team::new();
  team.front.take_reports();
  let [task, waiting, ..] = team.tasks;
  team.claimed(task);
  team.claimed(waiting);
  let run = team.succeeded(task);
  team.succeeded(waiting);
  team.merge_outside(&run);

  team.alice.block_on(team.alice.servers.end_claims(&team.programs));
  assert_eq!(team.front.taken(), [(task.to_string(), TaskPhase::Merged)]);
  assert!(team.on_server(task).claim.is_none());
  assert_eq!(
    team.held_here(),
    BTreeSet::from([waiting]),
    "work git does not find merged waits for review"
  );
}

#[test]
fn claim_lifecycle_the_heartbeat_keeps_only_the_claims_their_runs_did_not_end() {
  let mut team = Team::new();
  let [merged, cancelled, succeeded, _] = team.tasks;
  for task in [merged, cancelled, succeeded] {
    team.claimed(task);
  }
  team.merged(merged);
  team.succeeded(succeeded);
  // Cancelled while the server cannot be asked: its claim stays held here until a sync releases it.
  team.cancelled(cancelled);
  let (merged_at, succeeded_at) = (team.heartbeat_at(merged), team.heartbeat_at(succeeded));
  // The server's clock counts milliseconds: a heartbeat a moment later refreshes to a later time.
  std::thread::sleep(Duration::from_millis(5));

  // The server has no report: the merged claim stays held, and only the heartbeat could refresh it.
  team.sync(true);
  assert_eq!(team.held_here(), BTreeSet::from([merged, succeeded]));
  assert_eq!(team.heartbeat_at(merged), merged_at, "an ended claim is not kept fresh");
  assert!(team.heartbeat_at(succeeded) > succeeded_at, "a kept claim is");
  assert!(team.on_server(cancelled).claim.is_none(), "released by the sync");
}

#[test]
fn claim_lifecycle_a_server_without_the_report_refuses_it_as_not_supported_and_a_later_sync_ends_the_claim() {
  let mut team = Team::new();
  let [task, ..] = team.tasks;
  team.claimed(task);
  team.merged(task);

  let ended = team
    .alice
    .block_on(team.alice.server().end_claims(&team.programs))
    .expect("the server was asked");
  assert_eq!(ended.len(), 1);
  assert_eq!((ended[0].task, ended[0].end), (task, ClaimEnd::Complete));
  assert!(
    matches!(ended[0].result, Err(ClaimEndError::NotSupported)),
    "{:?}",
    ended[0].result
  );
  assert_eq!(team.held_here(), BTreeSet::from([task]), "not dropped");
  assert!(team.on_server(task).claim.is_some());
  assert!(team.front.taken().is_empty());

  team.front.take_reports();
  team.sync(false);
  assert_eq!(team.front.taken(), [(task.to_string(), TaskPhase::Merged)]);
  assert!(team.held_here().is_empty());
  assert!(team.on_server(task).claim.is_none());
}

/// Run again takes the claim before the runner records the new run, so the task's latest run here is still the
/// cancelled one: neither a run change nor a sync releases the claim taken again, and the heartbeat keeps it fresh.
#[test]
fn claim_lifecycle_a_claim_taken_again_after_a_cancelled_run_is_kept_for_the_run_that_starts() {
  let mut team = Team::new();
  let [task, ..] = team.tasks;
  team.claimed(task);
  team.cancelled(task);
  team.sync(false);
  assert!(
    team.on_server(task).claim.is_none(),
    "the cancelled run's claim is released"
  );

  team.claimed(task);
  let taken_at = team.heartbeat_at(task);
  std::thread::sleep(Duration::from_millis(5));
  team.alice.block_on(team.alice.servers.end_claims(&team.programs));
  team.sync(true);
  assert_eq!(team.held_here(), BTreeSet::from([task]));
  assert!(team.heartbeat_at(task) > taken_at, "kept fresh");
}

/// Run again a moment after the cancel, before a sync released the cancelled run's claim, finds it held and only
/// confirms it: the claim keeps its start, and work that asked for it after the run ended keeps it all the same. Once
/// that work does not start, the claim is the cancelled run's again.
#[test]
fn claim_lifecycle_a_claim_confirmed_after_a_cancelled_run_is_kept_until_its_work_does_not_start() {
  let mut team = Team::new();
  let [task, ..] = team.tasks;
  team.claimed(task);
  team.cancelled(task);
  std::thread::sleep(Duration::from_millis(5));

  let again = team.work_on(task);
  assert!(!again.newly_taken(), "held since before the cancel");
  team.sync(true);
  assert!(team.on_server(task).claim.is_some(), "kept for the run that starts");
  assert_eq!(team.held_here(), BTreeSet::from([task]));

  team.alice.block_on(again.release(&team.alice.servers));
  team.sync(false);
  assert!(team.on_server(task).claim.is_none(), "released with the cancelled run");
}

/// A claim Alice's other computer took after the run here was cancelled is that computer's work: syncs here keep it.
#[test]
fn claim_lifecycle_a_claim_taken_on_the_other_computer_after_the_run_here_ended_is_kept() {
  let mut team = Team::new();
  let [task, ..] = team.tasks;
  team.claimed(task);
  team.cancelled(task);
  team.sync(false);
  assert!(team.on_server(task).claim.is_none());
  std::thread::sleep(Duration::from_millis(5));

  team.claimed_elsewhere(task);
  team.sync(false);
  team.sync(false);
  assert!(team.on_server(task).claim.is_some(), "the other computer's claim stays");
  assert_eq!(team.held_here(), BTreeSet::from([task]));
}
