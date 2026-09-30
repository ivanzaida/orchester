//! Handoffs: the step result's optional handoff lists, and the task memory records a finished
//! step's handoff leaves. Run with `cargo test -p orchester-core --test memory handoff`.

use orchester_core::{
  id::{ResourceId, StepExecutionId, TaskId},
  memory::{
    AgentProvenance, Confidence, Handoff, HandoffWrite, Importance, Memory, MemoryAuthor, MemoryChanges, MemoryKind,
    MemoryScope, MemorySettings, MemoryStatus, SecretScreen, StepFate, WritePolicy,
  },
  resource::StepKey,
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
fn handoff_malformed_lists_never_cost_the_outcome() {
  let parse = |text: &str| StepResult::parse(text).expect("the outcome is read");
  let single = parse(r#"{"outcome":"failed","summary":"Broke.","completed":"one string"}"#);
  assert_eq!(
    (single.outcome, single.completed),
    (StepOutcome::Failed, vec!["one string".to_owned()])
  );
  let objects = parse(r#"{"outcome":"question","summary":"Which?","decisions":[{"decision":"x"}, "Keep y."]}"#);
  assert_eq!(
    (objects.outcome, objects.decisions),
    (StepOutcome::Question, vec!["Keep y.".to_owned()])
  );
  let nulls = parse(r#"{"outcome":"failed","summary":"Broke.","open_questions":[null, " ", 3]}"#);
  assert!(nulls.open_questions.is_empty());
  let unknown = parse(r#"{"outcome":"failed","summary":"Broke.","next_steps":["x"]}"#);
  assert_eq!(unknown.outcome, StepOutcome::Failed);
  let wrong_type = parse(r#"{"outcome":"done","summary":"Done.","next_agent_should_know":{"a":1}}"#);
  assert!(wrong_type.next_agent_should_know.is_empty());
  assert_eq!(
    StepResult::parse(r#"{"outcome":"done"}"#),
    None,
    "the summary is required"
  );
  assert_eq!(StepResult::parse(r#"{"outcome":"done","summary":7}"#), None);
}

struct Keeping {
  task: TaskId,
  screen: SecretScreen,
  settings: MemorySettings,
}

impl Keeping {
  fn new() -> Self {
    Self {
      task: TaskId::generate(),
      screen: SecretScreen::new(Vec::new()),
      settings: MemorySettings::default(),
    }
  }

  fn changes(&self, text: &str, (agent, fate): (&AgentProvenance, StepFate), earlier: Vec<Memory>) -> MemoryChanges {
    let result = StepResult::parse(text).expect("a result");
    let write = HandoffWrite {
      task: self.task,
      provenance: agent,
      sources: &[12],
      fate,
      settings: &self.settings,
      screen: &self.screen,
      now: at(5),
    };
    Handoff::from_result(&result)
      .expect("a handoff")
      .changes(&write, earlier)
      .expect("no secrets")
  }
}

/// A later execution of the same step as `agent`.
fn again(agent: &AgentProvenance) -> AgentProvenance {
  AgentProvenance {
    execution: StepExecutionId::generate(),
    ..agent.clone()
  }
}

fn kinds(records: &[Memory]) -> Vec<(MemoryKind, MemoryStatus)> {
  records
    .iter()
    .map(|record| (record.content().kind(), record.status()))
    .collect()
}

#[test]
fn handoff_of_a_passed_step_becomes_task_memory_with_its_decisions() {
  let keeping = Keeping::new();
  let agent = provenance(ResourceId::generate());
  let added = keeping.changes(RESULT, (&agent, StepFate::Passed), Vec::new()).added;
  assert_eq!(
    added[0].content().text(),
    "Added the memory store.\n\nCompleted:\n- Tables and FTS5 indexes\n\nDecisions:\n- Use the trigram tokenizer \
     for identifiers.\n\nThe next agent should know:\n- Run the store tests with a name filter."
  );
  assert_eq!(
    kinds(&added),
    [
      (MemoryKind::Handoff, MemoryStatus::Active),
      (MemoryKind::Decision, MemoryStatus::Active)
    ]
  );
  for record in &added {
    assert_eq!(record.scope(), MemoryScope::Task(keeping.task));
    assert_eq!(record.author(), &MemoryAuthor::Agent(agent.clone()));
    assert_eq!(record.sources(), [12]);
    assert_eq!(record.confidence(), Confidence::Medium);
    assert_eq!(record.content().importance(), Importance::High);
  }
  assert_eq!(added[1].content().text(), "Use the trigram tokenizer for identifiers.");

  let approval = Keeping {
    settings: MemorySettings {
      task_writes: WritePolicy::AfterApproval,
      ..MemorySettings::default()
    },
    ..Keeping::new()
  };
  let proposed = approval.changes(RESULT, (&agent, StepFate::Passed), Vec::new()).added;
  assert!(proposed.iter().all(|record| record.status() == MemoryStatus::Proposed));
}

#[test]
fn handoff_of_a_failed_step_is_a_failed_attempt_without_decisions() {
  let keeping = Keeping::new();
  let added = keeping
    .changes(
      RESULT,
      (&provenance(ResourceId::generate()), StepFate::Failed),
      Vec::new(),
    )
    .added;
  assert_eq!(kinds(&added), [(MemoryKind::Handoff, MemoryStatus::Active)]);
  let text = added[0].content().text();
  assert!(text.starts_with("This attempt failed;"), "{text}");
  assert!(!text.contains("Decisions:"), "{text}");
}

#[test]
fn handoff_replaces_the_steps_earlier_handoff_and_decisions() {
  let keeping = Keeping::new();
  let first = provenance(ResourceId::generate());
  let earlier = keeping.changes(RESULT, (&first, StepFate::Passed), Vec::new()).added;
  let other_step = AgentProvenance {
    step: StepKey::new("review").expect("key"),
    ..again(&first)
  };
  let other = keeping
    .changes(RESULT, (&other_step, StepFate::Passed), Vec::new())
    .added;
  let mut all = earlier.clone();
  all.extend(other);

  let retried = keeping.changes(RESULT, (&again(&first), StepFate::Passed), all.clone());
  assert_eq!(
    kinds(&retried.changed),
    [
      (MemoryKind::Handoff, MemoryStatus::Superseded),
      (MemoryKind::Decision, MemoryStatus::Deprecated)
    ],
    "only the step's own earlier records change"
  );
  assert_eq!(retried.changed[0].id(), earlier[0].id());
  assert_eq!(retried.added[0].supersedes(), Some(earlier[0].id()));

  // A failed attempt replaces the handoff but keeps the decisions of the step's last pass.
  let failed = keeping.changes(RESULT, (&again(&first), StepFate::Failed), all);
  assert_eq!(
    kinds(&failed.changed),
    [(MemoryKind::Handoff, MemoryStatus::Superseded)]
  );
  assert_eq!(kinds(&failed.added), [(MemoryKind::Handoff, MemoryStatus::Active)]);
}

#[test]
fn handoff_proposals_wait_and_earlier_proposals_are_withdrawn() {
  let keeping = Keeping {
    settings: MemorySettings {
      task_writes: WritePolicy::AfterApproval,
      ..MemorySettings::default()
    },
    ..Keeping::new()
  };
  let first = provenance(ResourceId::generate());
  let mut earlier = keeping.changes(RESULT, (&first, StepFate::Passed), Vec::new()).added;
  // The person approved the first handoff only; its decision still waits.
  earlier[0].approve(None, at(6)).expect("approve");
  let next = keeping.changes(RESULT, (&again(&first), StepFate::Passed), earlier.clone());
  assert_eq!(
    kinds(&next.changed),
    [(MemoryKind::Decision, MemoryStatus::Withdrawn)],
    "the waiting decision is withdrawn; the active handoff stays until the new one is approved"
  );
  assert_eq!(next.added[0].status(), MemoryStatus::Proposed);
  assert_eq!(next.added[0].supersedes(), Some(earlier[0].id()));
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
  let agent = provenance(ResourceId::generate());
  let settings = MemorySettings::default();
  let write = HandoffWrite {
    task: TaskId::generate(),
    provenance: &agent,
    sources: &[],
    fate: StepFate::Passed,
    settings: &settings,
    screen: &screen,
    now: at(1),
  };
  let refused = Handoff::from_result(&leaked)
    .expect("a handoff")
    .changes(&write, Vec::new())
    .expect_err("the secret is refused");
  assert_eq!(refused.secret, secret_name("DEPLOY_PASSWORD"));
  assert!(!refused.to_string().contains("31337"));
}
