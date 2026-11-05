//! A run's worktree is read, never written while the run has not ended; and a remote that wants credentials is
//! refused at once, git never waiting for a password.

use std::{
  collections::{BTreeMap, BTreeSet},
  io::{Read, Write},
  net::TcpListener,
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  orchestrator::run_git::{GitAccess, GitInvocation, RunGitAnswer, RunGitCall, RunGitRequest},
  resource::{CommandAction, CommandArg, CommandProgram, Pipeline, PromptText, Slug, Step, StepKey, StepKind, Tool},
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunSnapshot},
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;
use orchester_workspace::{orchestrator::git::RunGit, runner::RunRequestError};
use serde_json::json;

use super::scratch::{Scratch, refused};

fn name(value: &str) -> Name {
  Name::new(value).expect("name")
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

/// A pipeline of one command step, as runs pin it; what it runs does not matter here.
fn snapshot(db: &mut ProjectDb) -> RunSnapshot {
  let action = CommandAction::new(
    CommandProgram::new("git").expect("program"),
    vec![CommandArg::new("status").expect("arg")],
  );
  let tool = db
    .create_resource(
      Slug::new("check").expect("slug"),
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

/// The scratch repository as a project with a run at work in a worktree of its own on `orc-1-run`.
pub(super) fn working_run(scratch: &Scratch) -> Run {
  let root = scratch.root();
  let mut db = ProjectDb::open(&root).expect("the project's data");
  let snapshot = snapshot(&mut db);
  let details = TaskDetails {
    title: name("Work"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  let task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(5))
    .expect("task")
    .remove(0);
  let mut run = db
    .create_run(
      IdempotencyKey::generate(),
      NewRun {
        task: task.id(),
        snapshot,
        branch: BranchName::new("orc-1-run").expect("branch"),
        base: Revision::new(&scratch.head("HEAD")).expect("revision"),
        base_branch: None,
        base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
        budget: None,
        max_duration: None,
      },
      at(6),
    )
    .expect("run");
  let events = run.start(at(7)).expect("start");
  db.append_events(&run, &events).expect("events");
  let worktree = root.join(".orchester/worktrees").join(run.id().to_string());
  scratch.git(&[
    "worktree",
    "add",
    "-b",
    "orc-1-run",
    &worktree.to_string_lossy(),
    "main",
  ]);
  std::fs::write(worktree.join("agent.txt"), "the agent's work\n").expect("the agent's file");
  run
}

/// [`working_run`], whose one step then passed: the run succeeded and waits to be merged.
pub(super) fn succeeded_run(scratch: &Scratch) -> Run {
  let mut run = working_run(scratch);
  let mut db = ProjectDb::open(&scratch.root()).expect("the project's data");
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.start_step(execution, at(8)).expect("step");
  db.append_events(&run, &events).expect("events");
  let events = run.complete_step(execution, at(9)).expect("pass");
  db.append_events(&run, &events).expect("events");
  run
}

#[test]
fn orchestrator_git_reads_a_runs_worktree_and_never_writes_there_while_it_works() {
  let scratch = Scratch::new();
  let run = working_run(&scratch);
  let named = run.number().get().to_string();
  let status = match scratch.call("git_status", &json!({ "run": named }), false) {
    Ok(orchester_workspace::orchestrator::git::Called::Read(read)) => read,
    other => panic!("a read: {other:?}"),
  };
  assert_eq!(status["branch"], "orc-1-run", "{status}");
  assert_eq!(status["files"][0]["path"], "agent.txt");
  let log = scratch.read_tool("git_log", &json!({ "run": named }));
  assert_eq!(log["commits"][0]["subject"], "Initial commit", "{log}");

  for (name, arguments) in [
    (
      "git_commit",
      json!({ "run": run.id().to_string(), "message": "Mine", "all": true }),
    ),
    ("git_switch", json!({ "run": named, "branch": "main" })),
    ("git_propose", json!({ "run": named, "operation": { "kind": "clean" } })),
  ] {
    assert_eq!(refused(scratch.call(name, &arguments, true)), "run_active", "{name}");
  }
  let worktree = scratch.root().join(".orchester/worktrees").join(run.id().to_string());
  assert_eq!(
    scratch.git_in(&worktree, &["status", "--porcelain"]),
    "?? agent.txt",
    "the agent's work is untouched"
  );
  let asked = scratch.asked_runner();
  assert!(!asked.is_empty(), "the runner ran the reads");
  assert!(
    asked
      .iter()
      .all(|request| request.run == run.id() && request.access == GitAccess::Read),
    "{asked:?}"
  );
  assert!(
    !scratch.desktop_ran_in(&worktree),
    "the desktop never ran git in the run's worktree"
  );
}

/// The runner itself refuses a write, and git that does not only read, in the worktree of a run that works, whatever
/// the desktop asked.
#[test]
fn orchestrator_git_runner_refuses_writes_in_a_working_runs_worktree() {
  let scratch = Scratch::new();
  let run = working_run(&scratch);
  let git = |access, args: &[&str]| RunGitRequest {
    run: run.id(),
    access,
    call: RunGitCall::Git(GitInvocation {
      args: args.iter().map(|arg| (*arg).to_owned()).collect(),
      input: None,
      ssh: None,
      timeout_ms: 30_000,
      cap: None,
    }),
  };
  for request in [
    git(GitAccess::Write, &["status"]),
    git(GitAccess::Read, &["commit", "-m", "Mine"]),
    git(GitAccess::Read, &["symbolic-ref", "HEAD", "refs/heads/main"]),
    git(GitAccess::Read, &["remote", "add", "far", "https://example.invalid"]),
  ] {
    assert!(
      matches!(
        scratch.runs.call(request.clone()),
        Err(RunRequestError::Rejected(Rejection::RunNotEnded))
      ),
      "{request:?}"
    );
  }
  assert!(matches!(
    scratch.runs.call(git(GitAccess::Read, &["status", "--porcelain"])),
    Ok(RunGitAnswer::Ran { .. })
  ));
}

/// Once the run ended, the chat writes in its worktree, and git there still runs in the runner.
#[test]
fn orchestrator_git_writes_in_an_ended_runs_worktree_through_the_runner() {
  let scratch = Scratch::new();
  let run = succeeded_run(&scratch);
  let named = run.number().get().to_string();
  let told = scratch.wrote("git_commit", &json!({ "run": named, "message": "Mine", "all": true }));
  assert_eq!(told["branch"], "orc-1-run", "{told}");
  let worktree = scratch.root().join(".orchester/worktrees").join(run.id().to_string());
  assert_eq!(scratch.git_in(&worktree, &["log", "-1", "--format=%s"]), "Mine");
  assert!(
    scratch
      .asked_runner()
      .iter()
      .any(|request| request.access == GitAccess::Write),
    "the write went to the runner"
  );
  assert!(
    !scratch.desktop_ran_in(&worktree),
    "the desktop never ran git in the run's worktree"
  );
}

/// A remote over HTTP that answers every request with 401, as one that wants a password.
fn wants_a_password() -> String {
  let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
  let address = listener.local_addr().expect("its address");
  thread::spawn(move || {
    for stream in listener.incoming() {
      let Ok(mut stream) = stream else { continue };
      let mut request = [0_u8; 4096];
      // What the request says does not matter: every one is answered the same.
      let _read = stream.read(&mut request);
      let answer = "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"kontur\"\r\nContent-Length: 0\r\n\
                    Connection: close\r\n\r\n";
      // A client that went away needs no answer.
      let _written = stream.write_all(answer.as_bytes());
    }
  });
  format!("http://{address}/kontur.git")
}

#[test]
fn orchestrator_git_remote_that_wants_credentials_fails_fast_as_auth_failed() {
  let scratch = Scratch::new();
  scratch.git(&["remote", "add", "private", &wants_a_password()]);
  let started = Instant::now();
  let called = scratch.call("git_fetch", &json!({ "remote": "private" }), true);
  assert_eq!(refused(called), "auth_failed");
  assert!(
    started.elapsed() < Duration::from_secs(20),
    "git asked nobody and waited for nothing: {:?}",
    started.elapsed()
  );
  let called = scratch.call("git_push", &json!({ "remote": "private" }), true);
  assert_eq!(refused(called), "auth_failed");
}
