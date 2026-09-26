//! Agent CLI invocations and the transcript of a run.

use std::{collections::BTreeMap, fs};

use orchester_core::{
  connection::stream::{AgentEvent, CommandStatus, InvocationOutcome, OutputStream},
  id::{ConnectionId, IdempotencyKey, ProjectId, StepExecutionId},
  money::{Currency, Money},
  name::Name,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
  },
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunSnapshot, TokenUsage},
  store::{ActivityStore, ResourceStore, RunStore, TaskStore},
  task::TaskDetails,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use rusqlite::Connection;
use tempfile::TempDir;

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn model() -> ModelId {
  ModelId::new("gpt-5.5").expect("valid model")
}

/// A project with a started run of a one-step pipeline.
fn started_run() -> (TempDir, ProjectDb, Run, StepExecutionId) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    name("Kontur"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("create project");
  let details = TaskDetails {
    title: name("Say hello"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: None,
  };
  let task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(1))
    .expect("task")[0]
    .id();
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    model(),
    ProviderSettings::default(),
  );
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, at(2))
    .expect("profile")
    .id();
  let profiles = BTreeMap::from([(
    builder,
    db.publish_draft::<AgentProfile>(builder, at(3)).expect("publish"),
  )]);
  let step = Step {
    key: StepKey::new("implement").expect("key"),
    name: name("Implement"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
    },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline = Pipeline::new(vec![step]).expect("pipeline");
  let delivery = db
    .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, at(4))
    .expect("pipeline")
    .id();
  let version = db.publish_draft::<Pipeline>(delivery, at(5)).expect("publish");
  let (tools, instructions) = (BTreeMap::new(), BTreeMap::new());
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &tools,
    instructions: &instructions,
  };
  let new = NewRun {
    task,
    snapshot: RunSnapshot::build(delivery, &version, published).expect("snapshot"),
    branch: BranchName::new("orchester/orc-1-run-1").expect("branch"),
    base: Revision::new(&"a".repeat(40)).expect("revision"),
    budget: Money::from_micros(1_000_000, Currency::USD),
  };
  let mut run = db.create_run(IdempotencyKey::generate(), new, at(6)).expect("run");
  let events = run.start(at(7)).expect("start");
  db.append_events(&run, &events).expect("append");
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  (repo, db, run, execution)
}

#[test]
fn activity_invocations_are_numbered_per_execution_and_saved() {
  let (_repo, mut db, run, execution) = started_run();
  let mut first = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start");
  assert_eq!(first.number, 1);
  first.session = Some("thread-1".into());
  first.usage = TokenUsage {
    input: 12_166,
    cached_input: 2_432,
    output: 5,
  };
  first.ended_at = Some(at(20));
  first.outcome = Some(InvocationOutcome::TurnFailed { message: "no".into() });
  db.save_invocation(&first).expect("save");
  let second = db
    .start_invocation(run.id(), execution, &model(), at(30))
    .expect("start");
  assert_eq!(second.number, 2);
  let stored = db.invocations(run.id()).expect("read");
  assert_eq!(stored, [first, second]);
  assert_eq!(stored[0].active_millis(at(99)), 10);
  assert_eq!(stored[1].active_millis(at(35)), 5);
}

#[test]
fn activity_transcript_round_trips_in_order_and_reads_after_a_sequence() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start");
  let events = vec![
    (
      at(11),
      AgentEvent::SessionStarted {
        session: "thread-1".into(),
      },
    ),
    (at(12), AgentEvent::Message { text: "Working".into() }),
    (
      at(13),
      AgentEvent::CommandFinished {
        item: "item_1".into(),
        command: "git status".into(),
        output: "clean".into(),
        exit_code: Some(0),
        status: CommandStatus::Completed,
      },
    ),
  ];
  assert_eq!(db.append_activity(&invocation, &events[..2]).expect("append"), 2);
  assert_eq!(db.append_activity(&invocation, &events[2..]).expect("append"), 3);
  let raw = (
    at(14),
    AgentEvent::Raw {
      stream: OutputStream::Stderr,
      line: "warning".into(),
    },
  );
  assert_eq!(
    db.append_activity(&invocation, std::slice::from_ref(&raw))
      .expect("append"),
    4
  );
  let all = db.activity(run.id(), 0).expect("read");
  let read: Vec<_> = all.iter().map(|entry| (entry.at, entry.event.clone())).collect();
  let mut expected = events.clone();
  expected.push(raw);
  assert_eq!(read, expected);
  assert!(
    all
      .iter()
      .all(|entry| entry.invocation == invocation.id && entry.execution == execution)
  );
  let sequences: Vec<_> = db
    .activity(run.id(), 2)
    .expect("read")
    .iter()
    .map(|entry| entry.sequence)
    .collect();
  assert_eq!(sequences, [3, 4]);
}

#[test]
fn activity_transcript_is_append_only() {
  let (repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start");
  db.append_activity(&invocation, &[(at(11), AgentEvent::TurnStarted)])
    .expect("append");
  let raw = Connection::open(repo.path().join(".orchester/orchester.db")).expect("open");
  assert!(raw.execute("UPDATE agent_events SET kind = 'x'", []).is_err());
  assert!(raw.execute("DELETE FROM agent_events", []).is_err());
}
