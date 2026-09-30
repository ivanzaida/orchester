//! Handoffs: the step result's optional handoff lists, and the task memory records a finished
//! step's handoff leaves. Run with `cargo test -p orchester-core --test memory handoff`.

use orchester_core::{
  id::{ResourceId, TaskId},
  memory::{
    Confidence, Handoff, Importance, MemoryAuthor, MemoryKind, MemoryScope, MemorySettings, MemoryStatus, SecretScreen,
    WritePolicy,
  },
  run::{STEP_RESULT_SCHEMA, StepOutcome, StepResult},
};

use super::support::{at, provenance, secret_name, secret_value};

const RESULT: &str = r#"{"outcome":"done","summary":"Added the memory store.",
  "completed":["Tables and FTS5 indexes"],
  "decisions":["Use the trigram tokenizer for identifiers.", "  "],
  "open_questions":null,
  "next_agent_should_know":["Run the store tests with a name filter."]}"#;

#[test]
fn handoff_lists_are_optional_and_may_be_null() {
  let plain = StepResult::parse(r#"{"outcome":"done","summary":"Done."}"#).expect("a result");
  assert!(plain.decisions.is_empty() && plain.completed.is_empty());
  let full = StepResult::parse(RESULT).expect("a result");
  assert_eq!(full.completed, ["Tables and FTS5 indexes"]);
  assert!(full.open_questions.is_empty());
  assert_eq!(StepResult::parse(r#"{"outcome":"done","summary":"x","next":[]}"#), None);
  let schema: serde_json::Value = serde_json::from_str(STEP_RESULT_SCHEMA).expect("the schema is JSON");
  let required: Vec<&str> = schema["required"]
    .as_array()
    .expect("required")
    .iter()
    .filter_map(serde_json::Value::as_str)
    .collect();
  assert_eq!(
    required,
    [
      "outcome",
      "summary",
      "completed",
      "decisions",
      "open_questions",
      "next_agent_should_know"
    ],
    "Codex's structured output requires every property"
  );
}

#[test]
fn handoff_becomes_task_memory_with_its_decisions() {
  let result = StepResult::parse(RESULT).expect("a result");
  let handoff = Handoff::from_result(&result).expect("a handoff");
  assert_eq!(
    handoff.text(),
    "Added the memory store.\n\nCompleted:\n- Tables and FTS5 indexes\n\nDecisions:\n- Use the trigram tokenizer \
     for identifiers.\n\nThe next agent should know:\n- Run the store tests with a name filter."
  );
  let task = TaskId::generate();
  let agent = provenance(ResourceId::generate());
  let screen = SecretScreen::new(Vec::new());
  let settings = MemorySettings::default();
  let records = handoff
    .records(task, (&agent, &[12]), (&settings, &screen), at(5))
    .expect("no secrets");
  let kinds: Vec<MemoryKind> = records.iter().map(|record| record.content().kind()).collect();
  assert_eq!(kinds, [MemoryKind::Handoff, MemoryKind::Decision]);
  for record in &records {
    assert_eq!(record.scope(), MemoryScope::Task(task));
    assert_eq!(record.status(), MemoryStatus::Active);
    assert_eq!(record.author(), &MemoryAuthor::Agent(agent.clone()));
    assert_eq!(record.sources(), [12]);
    assert_eq!(record.confidence(), Confidence::Medium);
    assert_eq!(record.content().importance(), Importance::High);
  }
  assert_eq!(
    records[1].content().text(),
    "Use the trigram tokenizer for identifiers."
  );

  let approval = MemorySettings {
    task_writes: WritePolicy::AfterApproval,
    ..settings
  };
  let proposed = handoff
    .records(task, (&agent, &[]), (&approval, &screen), at(5))
    .expect("no secrets");
  assert!(proposed.iter().all(|record| record.status() == MemoryStatus::Proposed));
}

#[test]
fn handoff_of_a_question_or_holding_a_secret_is_not_kept() {
  let question = StepResult::parse(r#"{"outcome":"question","summary":"Which one?"}"#).expect("a result");
  assert_eq!(Handoff::from_result(&question), None);
  let failed = StepResult::parse(r#"{"outcome":"failed","summary":"Tests fail."}"#).expect("a result");
  assert_eq!(failed.outcome, StepOutcome::Failed);
  assert!(Handoff::from_result(&failed).is_some());

  let leaked = StepResult::parse(
    r#"{"outcome":"done","summary":"Done.","decisions":["Deploy with fake-pass-31337."],"completed":null,
    "open_questions":null,"next_agent_should_know":null}"#,
  )
  .expect("a result");
  let screen = SecretScreen::new(vec![(secret_name("DEPLOY_PASSWORD"), secret_value("fake-pass-31337"))]);
  let refused = Handoff::from_result(&leaked)
    .expect("a handoff")
    .records(
      TaskId::generate(),
      (&provenance(ResourceId::generate()), &[]),
      (&MemorySettings::default(), &screen),
      at(1),
    )
    .expect_err("the secret is refused");
  assert_eq!(refused.secret, secret_name("DEPLOY_PASSWORD"));
  assert!(!refused.to_string().contains("31337"));
}
