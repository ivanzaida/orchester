//! The memory area of the coordinator link (`docs/architecture/remote-runners.md`, "Rules of the boundary"): its
//! requests and answers come back the same from their JSON, and on a fixture project the records the link chooses for
//! a step, asked and answered through JSON, are the records and the order the project database gives the prompt
//! itself ([`next_invocation_memory`]). Run with `cargo test -p orchester-runner --test coordinator_link_memory`.

use std::{collections::BTreeSet, fmt::Debug, path::Path};

use orchester_core::{
  id::{IdempotencyKey, ProjectId, ResourceId, RunId, StepExecutionId, TaskId},
  memory::{
    AgentProvenance, AgentWrite, Confidence, Delivery, Importance, InjectionReason, Memory, MemoryContent, MemoryKey,
    MemoryKind, MemoryLimits, MemoryScope, MemorySettings, MemoryStore, RecallRequest as MemoryRecall, Recalled,
    SecretScreen, StepContext, StepRecall, Tags, UnreadableMemory, WritePolicy, next_invocation_memory, recall,
  },
  name::Name,
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{PromptText, RepositoryAccess, Step, StepKey, StepKind},
  store::TaskStore,
  task::{Task, TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_runner::execution::{
  LocalLinks,
  link::{
    LinkOpener,
    memory::{RecallRequest, SettingsRequest, UnreadableMemories, UnreadableRequest},
  },
};
use orchester_store::ProjectDb;
use serde::{Serialize, de::DeserializeOwned};

/// `value`, written as JSON and read back.
fn through_json<T: Serialize + DeserializeOwned>(value: &T) -> T {
  let json = serde_json::to_string(value).expect("the value serializes");
  serde_json::from_str(&json).expect("the JSON reads back")
}

fn round_trip<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: &T) {
  assert_eq!(&through_json(value), value);
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn name(value: &str) -> Name {
  Name::new(value).expect("a name")
}

fn content(kind: MemoryKind, text: &str) -> MemoryContent {
  MemoryContent::new(kind, text).expect("content")
}

fn person(scope: MemoryScope, content: MemoryContent, millis: i64) -> Memory {
  let screened = SecretScreen::new(Vec::new()).check(content).expect("no secrets");
  Memory::by_person(scope, screened, at(millis))
}

/// A record the Builder profile's agent wrote in an earlier step of the task.
fn by_agent(scope: MemoryScope, content: MemoryContent, profile: ResourceId, millis: i64) -> Memory {
  let write = AgentWrite {
    provenance: AgentProvenance {
      profile,
      run: RunId::generate(),
      step: StepKey::new("plan").expect("a key"),
      execution: StepExecutionId::generate(),
    },
    confidence: Confidence::Medium,
    sources: vec![3, 4],
  };
  let screened = SecretScreen::new(Vec::new()).check(content).expect("no secrets");
  Memory::by_agent(scope, write, screened, &MemorySettings::DEFAULT, at(millis))
}

fn step() -> Step {
  Step {
    key: StepKey::new("implement").expect("a key"),
    name: name("Implement the migration"),
    kind: StepKind::Agent {
      profile: Some(ResourceId::generate()),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Move the billing tables to the new schema and keep the importer working.")
      .expect("a prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

/// A project whose task has more memory than the limits let a prompt have: its latest handoff (long enough to be
/// cut), an older one, pinned project records, the task's decisions and warnings, records that match the step's text
/// and records that do not, with keys, tags and every importance.
fn fixture(root: &Path) -> (ProjectDb, Task) {
  let status = std::process::Command::new("git")
    .arg("-C")
    .arg(root)
    .args(["init", "--quiet"])
    .status()
    .expect("run git");
  assert!(status.success(), "git init: {status}");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("a key"),
    name("Kontur"),
  );
  let mut db = ProjectDb::create(root, project).expect("create the project");
  let details = TaskDetails {
    title: name("Billing schema migration"),
    brief: "Move the billing tables and the invoice importer to the new schema.".to_owned(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  let task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(1))
    .expect("a task")
    .remove(0);
  let records = records(MemoryScope::Task(task.id()), None, 10);
  db.add_memories(&records).expect("the records are added");
  db.save_memory_settings(&limits()).expect("the setting is saved");
  (db, task)
}

/// Fewer records than the fixture's records need.
fn limits() -> MemorySettings {
  MemorySettings {
    task_writes: WritePolicy::Automatic,
    project_writes: WritePolicy::AfterApproval,
    limits: MemoryLimits::new(4, 600).expect("valid limits"),
  }
}

/// The fixture's records, of the task's `scope`: written by a person, or by the agent of `profile` where one is
/// given (an agent's records name the run they were written in, which a project's database holds only for real runs).
/// The latest handoff has `handoff_lines` lines of detail: 120 do not fit [`limits`], 10 do.
fn records(scope: MemoryScope, profile: Option<ResourceId>, handoff_lines: usize) -> Vec<Memory> {
  let written = |content: MemoryContent, millis| match profile {
    Some(profile) => by_agent(scope, content, profile, millis),
    None => person(scope, content, millis),
  };
  let long_handoff = format!(
    "Summary: the importer reads both schemas.\n{}",
    "Moved table billing_rows.\n".repeat(handoff_lines)
  );
  let mut pinned = person(
    MemoryScope::Project,
    content(MemoryKind::Fact, "Migrations run in one transaction."),
    4,
  );
  pinned.set_pinned(true, at(5)).expect("pinned");
  let tags = Tags::new(["builder", "schema"]).expect("tags");
  vec![
    written(content(MemoryKind::Handoff, "Summary: an older handoff."), 2),
    written(content(MemoryKind::Handoff, &long_handoff), 3),
    pinned,
    person(
      scope,
      content(
        MemoryKind::Decision,
        "Keep the old billing tables readable until the release.",
      ),
      6,
    ),
    person(
      scope,
      content(MemoryKind::Warning, "The invoice importer locks the billing tables.").with_importance(Importance::High),
      7,
    ),
    person(
      MemoryScope::Project,
      content(MemoryKind::Fact, "The schema migration tool lives in tools/migrate.")
        .with_key(MemoryKey::new("tools.migrate").expect("a key"))
        .with_tags(tags),
      8,
    ),
    written(
      content(MemoryKind::Observation, "The billing importer retries twice."),
      9,
    ),
    person(
      MemoryScope::Project,
      content(MemoryKind::Fact, "Invoices are numbered per year.").with_importance(Importance::Low),
      10,
    ),
    person(
      MemoryScope::Project,
      content(MemoryKind::Fact, "Deploys run on Fridays."),
      11,
    ),
  ]
}

#[test]
fn coordinator_link_memory_requests_and_answers_round_trip() {
  let task = TaskId::generate();
  let profile = ResourceId::generate();
  round_trip(&RecallRequest {
    step: StepRecall {
      task,
      title: "Billing schema migration".to_owned(),
      brief: "Move the billing tables.".to_owned(),
      step: "Implement".to_owned(),
      instructions: "Keep the importer working.".to_owned(),
      profile,
      role: "Builder".to_owned(),
    },
    at: at(42),
  });
  round_trip(&UnreadableRequest);
  round_trip(&SettingsRequest);
  round_trip(&UnreadableMemories {
    records: vec![UnreadableMemory {
      id: "not-an-id".to_owned(),
      detail: "a memory key is one line".to_owned(),
    }],
  });
  round_trip(&limits());
  round_trip(&Recalled::default());
  // Records of an agent and of a person, given whole, cut and left out.
  let request = MemoryRecall {
    task,
    profile,
    role: "Builder",
    limits: limits().limits,
    now: at(86_400_000),
  };
  let recalled = recall(
    &request,
    records(MemoryScope::Task(task), Some(profile), 120),
    Vec::new(),
  );
  assert!(recalled.entries().iter().any(|entry| entry.truncated));
  assert!(!recalled.left_out().is_empty());
  round_trip(&recalled);
}

#[test]
fn coordinator_link_memory_limits_read_back_only_within_their_range() {
  let json = r#"{"records":0,"tokens":3000}"#;
  assert!(serde_json::from_str::<MemoryLimits>(json).is_err());
  let blank = r#"{"kind":"Fact","key":null,"text":"  ","tags":[],"importance":"Normal"}"#;
  assert!(serde_json::from_str::<MemoryContent>(blank).is_err());
}

#[test]
fn coordinator_link_memory_gives_the_prompt_the_records_the_project_database_chooses_in_its_order() {
  let repo = tempfile::tempdir().expect("a temporary directory");
  let (db, task) = fixture(repo.path());
  let step = step();
  let context = StepContext {
    task: &task,
    step: &step,
    profile: ResourceId::generate(),
    role: "Builder",
  };
  let now = at(86_400_000);
  let direct = next_invocation_memory(&db, context, now).expect("the database recalls");

  let link = LocalLinks::new(repo.path()).open().expect("a link");
  let request = through_json(&RecallRequest::for_step(context, now));
  let answer = through_json(&link.recall(&request).expect("the link recalls"));

  // The same records, bodies, texts and tokens, in the same order, and the same left out.
  assert_eq!(answer, direct);
  let reasons: Vec<InjectionReason> = answer.entries().iter().map(|entry| entry.reason).collect();
  assert_eq!(
    reasons,
    [
      InjectionReason::Handoff,
      InjectionReason::Pinned,
      InjectionReason::Warning,
      InjectionReason::Decision
    ]
  );
  assert!(
    answer
      .left_out()
      .iter()
      .any(|left| left.delivery == Delivery::OverRecordLimit),
    "the limits leave records out: {:?}",
    answer.left_out()
  );
}

#[test]
fn coordinator_link_memory_reads_the_setting_and_the_unreadable_records_of_the_project() {
  let repo = tempfile::tempdir().expect("a temporary directory");
  let (db, _) = fixture(repo.path());
  // A row damaged the way an edit outside Orchester would.
  let raw = rusqlite::Connection::open(repo.path().join(".orchester").join("orchester.db")).expect("open");
  raw
    .execute(
      "UPDATE memories SET key = 'two' || char(10) || 'lines' WHERE text = 'Deploys run on Fridays.'",
      [],
    )
    .expect("damage the row");
  let link = LocalLinks::new(repo.path()).open().expect("a link");
  assert_eq!(
    through_json(&link.settings(&SettingsRequest).expect("the setting")),
    db.memory_settings().expect("the setting")
  );
  let unreadable = through_json(&link.unreadable(&UnreadableRequest).expect("the scan"));
  assert_eq!(unreadable.records.len(), 1);
  assert_eq!(unreadable.records, db.unreadable_memories().expect("the scan"));
}
