//! The requests the runner's executor sends its coordinator for rows the desktop owns (ORC-680,
//! `docs/architecture/remote-runners.md`, rule 3): each request and each answer survives serialization unchanged, and
//! applied as it arrived it writes what the direct write wrote. `cargo test -p orchester-store --test owned_records
//! owned_record`.

mod support;

use orchester_core::{
  id::{ConnectionId, ResourceId},
  memory::{
    AgentProvenance, Handoff, Importance, MemoryContent, MemoryFilter, MemoryKey, MemoryKind, MemoryNote, MemoryStatus,
    MemoryStore, NoteScope, ScopeFilter, StepFate, Tags,
  },
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{ClassifiedCommand, CommandScope},
  },
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, Slug, StepKey},
  run::{
    ApprovalRuleId, ApprovalSubject, ProjectRuleId, ProjectScope, RuleScope, Run, StepOutcome, StepResult, ToolReach,
  },
  store::ResourceStore,
};
use orchester_store::{
  AlwaysAllowAnswer, AlwaysAllowRecord, MemoryAnswer, MemoryRecord, ModelRequirement, ProjectDb, RuleAnswer,
  RuleRecord, StoreError,
};
use serde::{Serialize, de::DeserializeOwned};

use self::support::{at, name, started_run};

/// `value` sent as JSON and read back, as the coordinator in another process, or the executor its answer, receives it.
fn sent<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: &T) -> T {
  let text = serde_json::to_string(value).expect("it serializes");
  let received: T = serde_json::from_str(&text).expect("it deserializes");
  assert_eq!(&received, value, "it survives being sent");
  received
}

fn mcp_rule(tool: &str) -> RuleScope {
  RuleScope::SimilarMcpTool {
    server: "pencil".to_owned(),
    tool: tool.to_owned(),
    folder: None,
    check: None,
  }
}

fn qa_rule(blob: &str) -> ProjectScope {
  ProjectScope::of(&ApprovalSubject::McpTool {
    server: "Orchester".to_owned(),
    tool: "start_qa_app".to_owned(),
    reach: ToolReach::Script { blob: blob.to_owned() },
  })
  .expect("a project rule")
}

fn provenance(run: &Run, execution: orchester_core::id::StepExecutionId) -> AgentProvenance {
  AgentProvenance {
    profile: ResourceId::generate(),
    run: run.id(),
    step: StepKey::new("implement").expect("key"),
    execution,
  }
}

#[test]
fn owned_record_rules_round_trip_and_answer_the_rule() {
  let (_repo, mut db, run, _) = started_run();
  let similar = sent(&RuleRecord::Run {
    run: run.id(),
    scope: mcp_rule("batch_design"),
    at: at(10),
  });
  let RuleAnswer::Run(rule) = sent(&db.write_rule(&similar).expect("written")) else {
    panic!("a run's rule answers its id");
  };
  // The same scope while one is active answers that one, as the direct write does.
  assert_eq!(db.write_rule(&similar).expect("again"), RuleAnswer::Run(rule));
  let project = sent(&RuleRecord::Project {
    scope: qa_rule("3f2a1b9c"),
    run: run.id(),
    request: "invocation:qa".to_owned(),
    at: at(11),
  });
  let RuleAnswer::Project(project_rule) = sent(&db.write_rule(&project).expect("written")) else {
    panic!("a project's rule answers its id");
  };
  let counted = sent(&RuleRecord::ProjectAnswered { rule: project_rule });
  assert_eq!(sent(&db.write_rule(&counted).expect("counted")), RuleAnswer::Counted);
  let rules = db.approval_rules(run.id()).expect("rules");
  assert_eq!((rules.len(), rules[0].id), (1, rule));
  let project_rules = db.project_approval_rules().expect("project rules");
  assert_eq!(
    (
      project_rules[0].id,
      project_rules[0].answered,
      project_rules[0].scope.script()
    ),
    (project_rule, 1, "3f2a1b9c")
  );
  // Only a rule of `start_qa_app` kept to a script arrives as a project's rule.
  let other = r#"{"kind":"project","scope":{"kind":"similar_mcp_tool","server":"pencil","tool":"x"},"#.to_owned()
    + &format!(r#""run":"{}","request":"r","at":1}}"#, run.id());
  assert!(serde_json::from_str::<RuleRecord>(&other).is_err());
  assert_eq!(
    sent(&RuleAnswer::Run(ApprovalRuleId::new(3))),
    RuleAnswer::Run(ApprovalRuleId::new(3))
  );
  assert_eq!(
    sent(&RuleAnswer::Project(ProjectRuleId::new(4))),
    RuleAnswer::Project(ProjectRuleId::new(4))
  );
}

fn listed(prefixes: &[&str]) -> AgentPermissions {
  let entries = prefixes
    .iter()
    .map(|prefix| CommandPrefix::new(prefix).expect("prefix"))
    .collect();
  AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::Allowed(AllowedCommands::new(entries).expect("list")),
    NetworkAccess::On,
  )
}

#[test]
fn owned_record_always_allow_publishes_the_prefix_and_keeps_the_rule() {
  let (repo, mut db, run, _) = started_run();
  let profile = AgentProfile::new(
    name("Designer"),
    String::new(),
    PromptText::new("Design.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("claude-opus-5-5").expect("model"),
    ProviderSettings::default(),
  )
  .with_permissions(listed(&["cargo test"]));
  let id = db
    .create_resource(Slug::new("designer").expect("slug"), name("Designer"), &profile, at(1))
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(id, at(2)).expect("v1");
  let scope = CommandScope::new(repo.path(), &[], None, None);
  let command = ClassifiedCommand::classify("cargo add serde --features derive", &scope);
  let prefix = CommandPrefix::new("cargo add").expect("prefix");
  let record = sent(&AlwaysAllowRecord::Command {
    run: run.id(),
    profile: id,
    command: command.clone(),
    prefix: prefix.clone(),
    at: at(4),
  });
  assert_eq!(
    sent(&db.write_always_allow(&record).expect("written")),
    AlwaysAllowAnswer::Allowed
  );
  let latest = db.latest_version::<AgentProfile>(id).expect("read").expect("v2");
  assert_eq!(latest.body().permissions(), &listed(&["cargo test", "cargo add"]));
  let rules = db.approval_rules(run.id()).expect("rules");
  assert_eq!(rules[0].scope, RuleScope::AlwaysCommand { prefix });
  // A prefix Always allow does not offer for the command is not available, and nothing is published.
  let refused = AlwaysAllowRecord::Command {
    run: run.id(),
    profile: id,
    command,
    prefix: CommandPrefix::new("rm").expect("prefix"),
    at: at(5),
  };
  assert_eq!(
    sent(&db.write_always_allow(&sent(&refused)).expect("answered")),
    AlwaysAllowAnswer::Unavailable
  );
  assert_eq!(db.approval_rules(run.id()).expect("rules").len(), 1);
  // A server that is not the project's is not available either.
  let tool = sent(&AlwaysAllowRecord::McpTool {
    server: "gone".to_owned(),
    tool: "batch_design".to_owned(),
  });
  assert_eq!(
    db.write_always_allow(&tool).expect("answered"),
    AlwaysAllowAnswer::Unavailable
  );
}

#[test]
fn owned_record_model_requirement_round_trips_and_names_a_removed_connection() {
  let (_repo, mut db, ..) = started_run();
  let connection = ConnectionId::generate();
  let requirement = sent(&ModelRequirement::new(
    connection,
    &ModelId::new("gpt-5.5").expect("model"),
    "0.130.0",
  ));
  assert!(matches!(
    db.write_model_requirement(&requirement),
    Err(StoreError::ConnectionNotFound(id)) if id == connection
  ));
}

fn records(db: &ProjectDb, scope: ScopeFilter) -> Vec<orchester_core::memory::Memory> {
  db.memories(&MemoryFilter {
    scope,
    status: None,
    pinned_only: false,
  })
  .expect("records")
}

#[test]
fn owned_record_memory_keeps_a_note_and_a_handoff() {
  let (_repo, mut db, run, execution) = started_run();
  let content = MemoryContent::new(MemoryKind::Warning, "  The build needs nightly rustfmt. ")
    .expect("content")
    .with_key(MemoryKey::new("rustfmt").expect("key"))
    .with_tags(Tags::new(["Format", "check"]).expect("tags"))
    .with_importance(Importance::High);
  let note = sent(&MemoryRecord::Note {
    note: MemoryNote::new(NoteScope::Project, content),
    task: run.task(),
    provenance: provenance(&run, execution),
    at: at(10),
  });
  let MemoryAnswer::Note { memory, status } = sent(&db.write_memory(&note).expect("kept")) else {
    panic!("a note answers its record");
  };
  // Project memory is kept after approval by default.
  assert_eq!(status, MemoryStatus::Proposed);
  let kept = records(&db, ScopeFilter::Project);
  assert_eq!((kept.len(), kept[0].id()), (1, memory));
  assert_eq!(kept[0].content().text(), "The build needs nightly rustfmt.");
  let result = StepResult {
    outcome: StepOutcome::Done,
    summary: "Routed the writes.".to_owned(),
    unverified: Vec::new(),
    completed: vec!["Rules".to_owned()],
    decisions: vec!["The coordinator writes them.".to_owned()],
    open_questions: Vec::new(),
    next_agent_should_know: Vec::new(),
  };
  let handoff = sent(&MemoryRecord::Handoff {
    handoff: Handoff::from_result(&result).expect("a handoff"),
    task: run.task(),
    provenance: provenance(&run, execution),
    sources: vec![7],
    fate: StepFate::Passed,
    at: at(11),
  });
  assert_eq!(sent(&db.write_memory(&handoff).expect("kept")), MemoryAnswer::Handoff);
  let task = records(&db, ScopeFilter::Task(run.task()));
  let kinds: Vec<_> = task.iter().map(|memory| memory.content().kind()).collect();
  assert_eq!(kinds.len(), 2);
  assert!(kinds.contains(&MemoryKind::Handoff) && kinds.contains(&MemoryKind::Decision));
  // A handoff that says nothing arrives as none.
  let blank = r#"{"summary":" ","completed":[],"decisions":[],"open_questions":[],"next_agent_should_know":[]}"#;
  assert!(serde_json::from_str::<Handoff>(blank).is_err());
  let blank_item =
    r#"{"summary":"s","completed":[" "],"decisions":[],"open_questions":[],"next_agent_should_know":[]}"#;
  assert!(serde_json::from_str::<Handoff>(blank_item).is_err());
}
