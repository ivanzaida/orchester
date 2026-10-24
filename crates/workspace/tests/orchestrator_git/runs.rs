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
  id::{IdempotencyKey, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{CommandAction, CommandArg, CommandProgram, Pipeline, PromptText, Slug, Step, StepKey, StepKind, Tool},
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunSnapshot},
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::{ProjectDb, exclude_project_data};
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
fn working_run(scratch: &Scratch) -> Run {
  let root = scratch.root();
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    name("Kontur"),
  );
  let mut db = ProjectDb::create(&root, project).expect("project");
  exclude_project_data(&root).expect("the project's data stays out of git");
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
