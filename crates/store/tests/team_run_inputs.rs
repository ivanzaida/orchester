//! Team runs and their inputs (team authority P3): a team task's identity without its definition, team inputs kept by
//! the operation that accepted them before their run starts, a run created with them, executed and read back after a
//! restart with no team task row, inputs that never change and hold no secret value, usage and plan limits that name
//! a team run's task from its inputs, and a local run that keeps its inputs as today.
mod support;

#[path = "team_run_inputs/accepted.rs"]
mod accepted;

use std::{collections::BTreeMap, fs, num::NonZeroU32, path::Path};

use orchester_core::{
  blob::ContentHash,
  connection::{
    AgentProvider,
    plan_limits::{LimitReading, PlanLimits},
  },
  id::{
    ClaimId, ConnectionId, IdempotencyKey, ModuleId, OperationId, ProjectId, ResourceId, SecretId, ServerId, TaskId,
    WorkId,
  },
  integration::base::RunBaseSource,
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, Deliverable, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
    VersionNumber,
  },
  revision::Revision,
  run::{
    BranchName, CommitId, NewRun, PublishedVersions, RunSnapshot, StepExecutor,
    bundle::{CanonicalExecutionInputs, PinnedBodies, ResourceLabel},
    inputs::{
      BoundReference, BundleContent, InputsOrigin, LocalBindings, LocalConsent, ProfileBinding, RunInputs, TaskAtStart,
      TeamAcceptance,
    },
  },
  sharing::TaskRevision,
  store::{ActivityStore, ResourceStore, RunStore, TaskStore},
  task::{
    TaskDetails, TaskLimits,
    identity::{TaskOwner, TeamTaskRef},
  },
};
use orchester_store::{AcceptedInputs, ProjectDb, RunStoreError};
use serde_json::Value;
use support::{at, model, name};
use tempfile::TempDir;

/// A project linked to a server project, with a published one-step pipeline and its profile.
struct Team {
  repo: TempDir,
  db: ProjectDb,
  server: ServerId,
  project: ProjectId,
  profile: ResourceId,
  snapshot: RunSnapshot,
  inputs: CanonicalExecutionInputs,
}

fn team_project() -> Team {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    name("Kontur"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("create project");
  let body = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    model(),
    ProviderSettings::default(),
  );
  let profile = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &body, at(2))
    .expect("profile")
    .id();
  let profile_version = db.publish_draft::<AgentProfile>(profile, at(3)).expect("publish");
  let step = Step {
    key: StepKey::new("implement").expect("key"),
    name: name("Implement"),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
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
  let profiles = BTreeMap::from([(profile, profile_version.clone())]);
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &BTreeMap::new(),
    instructions: &BTreeMap::new(),
    skills: &BTreeMap::new(),
  };
  let snapshot = RunSnapshot::build(delivery, &version, published).expect("snapshot");
  let bodies = PinnedBodies {
    labels: &[
      (profile_version.id(), label("builder", "Builder")),
      (version.id(), label("delivery", "Delivery")),
    ],
    profiles: &[profile_version],
    instructions: &[],
    tools: &[],
    skills: &[],
  };
  let inputs = CanonicalExecutionInputs::pin(&snapshot, bodies).expect("inputs");
  drop(db);
  let (server, project) = (ServerId::generate(), ProjectId::generate());
  connection(repo.path())
    .execute(
      "INSERT INTO sharing (only_row, server_id, project_id, project_key, role, joined_at, event_cursor, \
       last_synced_at) VALUES (1, ?1, ?2, 'ORC', 'member', 1, 0, 1)",
      [server.to_string(), project.to_string()],
    )
    .expect("link the project");
  let db = ProjectDb::open(repo.path()).expect("open");
  Team {
    repo,
    db,
    server,
    project,
    profile,
    snapshot,
    inputs,
  }
}

fn connection(repo: &Path) -> rusqlite::Connection {
  let connection = rusqlite::Connection::open(repo.join(".orchester").join("orchester.db")).expect("open");
  connection
    .pragma_update(None, "foreign_keys", true)
    .expect("references enforced");
  connection
}

impl Team {
  fn task(&self) -> TeamTaskRef {
    TeamTaskRef {
      server: self.server,
      project: self.project,
      task: TaskId::generate(),
    }
  }

  fn inputs(&self, origin: InputsOrigin, references: Vec<BoundReference>) -> RunInputs {
    let task = TaskAtStart {
      title: "Say hello".into(),
      brief: "Greet the team.".into(),
      criteria: vec!["It says hello".into()],
    };
    let content = BundleContent::new(
      origin,
      (NonZeroU32::new(7).expect("number"), task),
      (TaskLimits::NONE, Deliverable::default()),
      self.inputs.clone(),
    )
    .expect("content");
    let bindings = LocalBindings {
      profiles: vec![ProfileBinding {
        resource: self.profile,
        connection: ConnectionId::generate(),
        account: None,
        required_cli: None,
      }],
      references,
    };
    RunInputs::new(content, bindings, LocalConsent::default()).expect("inputs")
  }

  fn new_run(&self, task: TaskId) -> NewRun {
    NewRun {
      task,
      snapshot: self.snapshot.clone(),
      branch: BranchName::new("orchester/orc-1-run-1").expect("branch"),
      base: Revision::new(&"a".repeat(40)).expect("revision"),
      base_branch: None,
      base_source: RunBaseSource::CheckedOut,
      budget: None,
      max_duration: None,
    }
  }
}

fn accepted(task: TeamTaskRef, operation: OperationId) -> InputsOrigin {
  InputsOrigin::Team(TeamAcceptance {
    task,
    definition_revision: revision(3),
    work: WorkId::generate(),
    claim: ClaimId::generate(),
    operation,
    module: ModuleId::generate(),
    branch: BranchName::new("orchester/module-greetings").expect("branch"),
    base_commit: CommitId::new(&"b".repeat(40)).expect("commit"),
    bundle: ContentHash::new(&"c".repeat(64)).expect("hash"),
  })
}

fn revision(value: u64) -> TaskRevision {
  TaskRevision::try_from(value).expect("revision")
}

/// What a pinned version was called when the inputs were accepted.
fn label(slug: &str, title: &str) -> ResourceLabel {
  ResourceLabel::new(Slug::new(slug).expect("slug"), name(title))
}

#[test]
fn team_run_inputs_execute_and_read_back_without_the_task_definition() {
  let mut team = team_project();
  let task = team.task();
  let id = team.db.record_team_task(task, revision(3)).expect("identity");
  assert_eq!(team.db.record_team_task(task, revision(4)).expect("again"), id);
  let identity = team.db.task_identity(id).expect("read").expect("identity");
  assert_eq!(
    identity.owner,
    TaskOwner::Team {
      task,
      recorded_revision: revision(3)
    }
  );
  assert_eq!(team.db.task(id).expect("read"), None, "no definition is kept");

  let inputs = team.inputs(accepted(task, OperationId::generate()), Vec::new());
  let mut run = team
    .db
    .create_run_with_inputs(IdempotencyKey::generate(), team.new_run(id), &inputs, (at(6), None))
    .expect("a team run");
  let events = run.start(at(7)).expect("start");
  team.db.append_events(&run, &events).expect("it executes");
  drop(team.db);

  // A cached definition of the task comes and goes; the run and its inputs stay.
  let raw = connection(team.repo.path());
  raw
    .execute(
      "INSERT INTO tasks (id, number, batch_key, title, brief, status, created_at, sharing) \
       VALUES (?1, 7, ?1, 'Say hello', '', 'queued', 1, 'shared')",
      [id.to_string()],
    )
    .expect("a cached definition");
  raw
    .execute("DELETE FROM tasks WHERE id = ?1", [id.to_string()])
    .expect("the definition goes");
  drop(raw);

  let mut db = ProjectDb::open(team.repo.path()).expect("restart");
  assert_eq!(db.run(run.id()).expect("read").expect("run"), run);
  assert_eq!(db.run_inputs(run.id()).expect("read"), Some(inputs));
  assert_eq!(db.runs_of_task(id).expect("runs").len(), 1);
  let events = run.pause(at(8)).expect("pause");
  db.append_events(&run, &events).expect("it goes on after a restart");
  assert_eq!(db.run(run.id()).expect("read").expect("run"), run);
}

#[test]
fn team_run_inputs_are_accepted_once_for_their_team_task() {
  let mut team = team_project();
  let task = team.task();
  let id = team.db.record_team_task(task, revision(3)).expect("identity");
  let operation = OperationId::generate();
  let inputs = team.inputs(accepted(task, operation), Vec::new());
  let key = IdempotencyKey::generate();
  let run = team
    .db
    .create_run_with_inputs(key, team.new_run(id), &inputs, (at(6), None))
    .expect("a team run");
  let retried = team
    .db
    .create_run_with_inputs(key, team.new_run(id), &inputs, (at(7), None))
    .expect("the same run");
  assert_eq!(retried, run);
  let again = team
    .db
    .create_run_with_inputs(IdempotencyKey::generate(), team.new_run(id), &inputs, (at(7), None));
  assert!(matches!(again, Err(RunStoreError::InputsMismatch { .. })), "{again:?}");

  let other = team.db.record_team_task(team.task(), revision(1)).expect("identity");
  let elsewhere = team.db.create_run_with_inputs(
    IdempotencyKey::generate(),
    team.new_run(other),
    &team.inputs(accepted(task, OperationId::generate()), Vec::new()),
    (at(8), None),
  );
  assert!(
    matches!(elsewhere, Err(RunStoreError::InputsMismatch { .. })),
    "{elsewhere:?}"
  );

  let stranger = TeamTaskRef {
    server: ServerId::generate(),
    ..team.task()
  };
  assert!(matches!(
    team.db.record_team_task(stranger, revision(1)),
    Err(orchester_store::StoreError::OtherServerProject)
  ));
}

#[test]
fn team_run_inputs_a_local_task_keeps_its_id_from_a_team_task() {
  let mut team = team_project();
  let task = team.task();
  connection(team.repo.path())
    .execute(
      "INSERT INTO task_identities (id, owner) VALUES (?1, 'local')",
      [task.task.to_string()],
    )
    .expect("a local task's identity");
  assert!(matches!(
    team.db.record_team_task(task, revision(1)),
    Err(orchester_store::StoreError::TaskIdTaken(id)) if id == task.task
  ));
  let identity = team.db.task_identity(task.task).expect("read").expect("identity");
  assert_eq!(identity.owner, TaskOwner::Local, "the local task's identity stays");
}

#[test]
fn team_run_inputs_never_change_and_their_references_hold() {
  let mut team = team_project();
  let task = team.task();
  let id = team.db.record_team_task(task, revision(3)).expect("identity");
  let inputs = team.inputs(accepted(task, OperationId::generate()), Vec::new());
  let run = team
    .db
    .create_run_with_inputs(IdempotencyKey::generate(), team.new_run(id), &inputs, (at(6), None))
    .expect("a team run");
  drop(team.db);
  let raw = connection(team.repo.path());
  let run_id = run.id().to_string();
  for sql in [
    "UPDATE run_bundles SET body = '{}' WHERE run_id = ?1",
    "DELETE FROM run_bundles WHERE run_id = ?1",
    "UPDATE run_local_bindings SET body = '{}' WHERE run_id = ?1",
    "DELETE FROM run_local_bindings WHERE run_id = ?1",
    "UPDATE run_local_consent SET body = '{}' WHERE run_id = ?1",
    "DELETE FROM run_local_consent WHERE run_id = ?1",
  ] {
    let error = raw.execute(sql, [&run_id]).expect_err(sql);
    assert!(error.to_string().contains("run inputs are immutable"), "{sql}: {error}");
  }
  let refused = raw
    .execute(
      "INSERT INTO run_local_bindings (run_id, body) VALUES (?1, '{}')",
      [TaskId::generate().to_string()],
    )
    .expect_err("inputs of no run");
  assert!(refused.to_string().contains("FOREIGN KEY"), "{refused}");
  let identity = id.to_string();
  let kept = raw
    .execute("DELETE FROM task_identities WHERE id = ?1", [&identity])
    .expect_err("a team identity is kept");
  assert!(kept.to_string().contains("never deleted"), "{kept}");
  let moved = raw
    .execute(
      "UPDATE task_identities SET team_task_id = ?2 WHERE id = ?1",
      [identity, TaskId::generate().to_string()],
    )
    .expect_err("a team identity keeps its task");
  assert!(moved.to_string().contains("keeps its id"), "{moved}");
}

/// Every string a value holds, at any depth.
fn strings(value: &Value, found: &mut Vec<String>) {
  match value {
    Value::String(text) => found.push(text.clone()),
    Value::Array(items) => items.iter().for_each(|item| strings(item, found)),
    Value::Object(fields) => fields.values().for_each(|field| strings(field, found)),
    _ => {}
  }
}

#[test]
fn team_run_inputs_name_secrets_by_id_only() {
  let mut team = team_project();
  let task = team.task();
  let id = team.db.record_team_task(task, revision(3)).expect("identity");
  let (named, local) = (SecretId::generate(), SecretId::generate());
  let inputs = team.inputs(
    accepted(task, OperationId::generate()),
    vec![BoundReference::Secret { named, local }],
  );
  let run = team
    .db
    .create_run_with_inputs(IdempotencyKey::generate(), team.new_run(id), &inputs, (at(6), None))
    .expect("a team run");
  drop(team.db);
  let raw = connection(team.repo.path());
  let (bindings, consent): (String, String) = raw
    .query_row(
      "SELECT bindings.body, consent.body FROM run_local_bindings AS bindings \
       JOIN run_local_consent AS consent USING (run_id) WHERE run_id = ?1",
      [run.id().to_string()],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .expect("stored");
  let mut found = Vec::new();
  for body in [&bindings, &consent] {
    strings(&serde_json::from_str(body).expect("json"), &mut found);
  }
  assert!(found.contains(&named.to_string()) && found.contains(&local.to_string()));
  for text in found {
    let is_id = text.parse::<SecretId>().is_ok();
    assert!(is_id || text == "secret", "only ids and kinds are stored: {text}");
  }
}

/// The local inputs of a run of `snapshot`, its profiles' bodies and the labels read from `db`.
fn local_inputs(db: &ProjectDb, snapshot: &RunSnapshot) -> RunInputs {
  let profiles: Vec<_> = snapshot
    .steps()
    .iter()
    .filter_map(|step| match step.executor() {
      StepExecutor::Agent { profile, .. } => Some(*profile),
      StepExecutor::Tool { .. } => None,
    })
    .collect();
  let versions: Vec<_> = profiles
    .iter()
    .map(|pinned| {
      db.versions::<AgentProfile>(pinned.resource)
        .expect("versions")
        .into_iter()
        .find(|version| version.id() == pinned.version)
        .expect("pinned version")
    })
    .collect();
  let labels: Vec<_> = std::iter::once(snapshot.pipeline())
    .chain(profiles.iter().copied())
    .map(|pinned| {
      let resource = db.resource(pinned.resource).expect("read").expect("the resource");
      (
        pinned.version,
        ResourceLabel::new(resource.slug().clone(), resource.name().clone()),
      )
    })
    .collect();
  let bodies = PinnedBodies {
    profiles: &versions,
    instructions: &[],
    tools: &[],
    skills: &[],
    labels: &labels,
  };
  let canonical = CanonicalExecutionInputs::pin(snapshot, bodies).expect("inputs");
  let task = TaskAtStart {
    title: "Say hello".into(),
    brief: "Greet.".into(),
    criteria: vec!["It greets".into()],
  };
  let content = BundleContent::new(
    InputsOrigin::Local,
    (NonZeroU32::new(2).expect("number"), task),
    (TaskLimits::NONE, Deliverable::default()),
    canonical,
  )
  .expect("content");
  let bindings = LocalBindings {
    profiles: profiles
      .iter()
      .map(|pinned| ProfileBinding {
        resource: pinned.resource,
        connection: ConnectionId::generate(),
        account: None,
        required_cli: None,
      })
      .collect(),
    references: Vec::new(),
  };
  RunInputs::new(content, bindings, LocalConsent::default()).expect("inputs")
}

#[test]
fn team_run_inputs_a_local_run_keeps_its_inputs_and_rules() {
  let (_repo, mut db, run, _) = support::started_run();
  let details = TaskDetails {
    title: name("Say hello"),
    brief: "Greet.".into(),
    acceptance_criteria: vec!["It greets".into()],
    pipeline: Some(run.snapshot().pipeline().resource),
    limits: TaskLimits::NONE,
  };
  let mut task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(10))
    .expect("task")
    .remove(0);
  task.queue(VersionNumber::FIRST).expect("queues");
  let queued = db.save_task(&task).expect("save").saved().expect("saved");
  let inputs = local_inputs(&db, run.snapshot());
  let new = |task| NewRun {
    task,
    snapshot: run.snapshot().clone(),
    branch: BranchName::new("orchester/orc-2-run-1").expect("branch"),
    base: Revision::new(&"a".repeat(40)).expect("revision"),
    base_branch: None,
    base_source: RunBaseSource::CheckedOut,
    budget: None,
    max_duration: None,
  };
  let recorded = db
    .create_run_with_inputs(
      IdempotencyKey::generate(),
      new(queued.id()),
      &inputs,
      (at(11), Some(queued.revision())),
    )
    .expect("a local run");
  assert_eq!(db.run_inputs(recorded.id()).expect("read"), Some(inputs.clone()));
  let started_with = db
    .task_for_run(queued.id(), recorded.id())
    .expect("read")
    .expect("task");
  assert_eq!(started_with.details().title, name("Say hello"));
  assert_eq!(
    db.run_inputs(run.id()).expect("read"),
    None,
    "a run made without inputs has none"
  );

  let draft_run = db.create_run_with_inputs(IdempotencyKey::generate(), new(run.task()), &inputs, (at(12), None));
  assert!(matches!(draft_run, Err(RunStoreError::TaskNotQueued(task)) if task == run.task()));
}

#[test]
fn team_run_inputs_cli_requirement_round_trips_and_older_json_stays_readable() {
  let mut team = team_project();
  let task = team.task();
  let id = team.db.record_team_task(task, revision(3)).expect("identity");
  for required_cli in [None, Some(orchester_core::connection::AgentProvider::CodexCli)] {
    let operation = OperationId::generate();
    let original = team.inputs(accepted(task, operation), Vec::new());
    let mut bindings = original.bindings().clone();
    bindings.profiles[0].required_cli = required_cli;
    let inputs = RunInputs::new(original.bundle().clone(), bindings, original.consent().clone()).expect("inputs");
    team.db.accept_team_inputs(id, &inputs, at(6)).expect("accepted");
    let read = team.db.accepted_team_inputs(operation).expect("read").expect("inputs");
    assert_eq!(read.inputs, inputs);
    let raw: String = connection(team.repo.path())
      .query_row(
        "SELECT bindings FROM accepted_run_inputs WHERE operation_id = ?1",
        [operation.to_string()],
        |row| row.get(0),
      )
      .expect("stored JSON");
    assert_eq!(
      raw.contains("required_cli"),
      required_cli.is_some(),
      "older rows omit the field"
    );
  }
}
