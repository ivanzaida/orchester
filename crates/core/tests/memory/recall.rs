//! Search terms and recall: which active records an agent's prompt gets, in what order, and the
//! record and token limits. Run with `cargo test -p orchester-core --test memory recall`.

use std::num::{NonZeroU16, NonZeroU32};

use orchester_core::{
  id::{ResourceId, TaskId},
  memory::{
    InjectionReason, Memory, MemoryKey, MemoryKind, MemoryLimits, MemoryMatch, MemoryQuery, MemoryScope,
    MemorySettings, RecallRequest, SecretScreen, Tags, estimated_tokens, recall,
  },
};

use super::support::{DAY, at, content, provenance, screened};

#[test]
fn recall_query_keeps_word_stems_paths_and_identifiers() {
  let query =
    MemoryQuery::from_text("Перевір міграції бази у `crates/core/src/run/result.rs`, then call parseStepResult. Go!");
  assert!(query.words().contains(&"міграц".to_owned()), "{query:?}");
  assert!(query.words().contains(&"бази".to_owned()), "{query:?}");
  assert!(query.words().contains(&"parsestepresu".to_owned()), "{query:?}");
  assert!(
    !query.words().iter().any(|word| word == "go" || word == "у"),
    "{query:?}"
  );
  assert!(
    query.fragments().contains(&"crates/core/src/run/result.rs".to_owned()),
    "{query:?}"
  );
  assert!(query.fragments().contains(&"parsestepresu".to_owned()), "{query:?}");
  let long = "word ".repeat(10) + &(0..100).map(|n| format!("term{n} ")).collect::<String>();
  assert_eq!(MemoryQuery::from_text(&long).words().len(), MemoryQuery::MAX_TERMS);
  assert!(MemoryQuery::from_text("a b").is_empty());
}

#[test]
fn recall_estimates_four_bytes_a_token() {
  assert_eq!(estimated_tokens(""), 0);
  assert_eq!(estimated_tokens("abcde"), 2);
  // Cyrillic letters take two bytes each.
  assert_eq!(estimated_tokens("міграція"), 4);
}

struct Fixture {
  task: TaskId,
  profile: ResourceId,
}

impl Fixture {
  fn new() -> Self {
    Self {
      task: TaskId::generate(),
      profile: ResourceId::generate(),
    }
  }

  fn request(&self, limits: MemoryLimits) -> RecallRequest<'static> {
    RecallRequest {
      task: self.task,
      profile: self.profile,
      role: "Code Reviewer",
      limits,
      now: at(30 * DAY),
    }
  }

  fn by_person(&self, kind: MemoryKind, text: &str, day: i64) -> Memory {
    Memory::by_person(MemoryScope::Task(self.task), screened(kind, text), at(day * DAY))
  }
}

fn matched(memory: Memory, words: f64, fragments: f64) -> MemoryMatch {
  MemoryMatch {
    memory,
    words,
    fragments,
  }
}

fn reasons(
  request: &RecallRequest<'_>,
  candidates: Vec<Memory>,
  matches: Vec<MemoryMatch>,
) -> Vec<(InjectionReason, String)> {
  recall(request, candidates, matches)
    .entries()
    .iter()
    .map(|entry| (entry.reason, entry.memory.content().text().to_owned()))
    .collect()
}

#[test]
fn recall_fills_tiers_in_order_with_only_active_records() {
  let fixture = Fixture::new();
  let request = fixture.request(MemoryLimits::DEFAULT);
  let old_handoff = fixture.by_person(MemoryKind::Handoff, "Planned the store.", 1);
  let handoff = fixture.by_person(MemoryKind::Handoff, "Built the store.", 2);
  let decision = fixture.by_person(MemoryKind::Decision, "Use FTS5.", 3);
  let warning = fixture.by_person(MemoryKind::Warning, "Never VACUUM during a run.", 4);
  let mut deprecated = fixture.by_person(MemoryKind::Decision, "Use pgvector.", 5);
  deprecated.deprecate(at(6 * DAY)).expect("deprecate");
  let mut pinned = Memory::by_person(
    MemoryScope::Project,
    screened(MemoryKind::Fact, "The repo uses 2-space indent."),
    at(0),
  );
  pinned.set_pinned(true, at(0)).expect("pin");
  let fact = fixture.by_person(MemoryKind::Fact, "Tokens are estimated from bytes.", 7);
  let other_task = Memory::by_person(
    MemoryScope::Task(TaskId::generate()),
    screened(MemoryKind::Fact, "Another task's fact."),
    at(7 * DAY),
  );
  let proposal = Memory::by_agent(
    MemoryScope::Project,
    provenance(fixture.profile),
    screened(MemoryKind::Fact, "An unapproved fact."),
    Vec::new(),
    &MemorySettings::default(),
    at(8 * DAY),
  );
  let chosen = reasons(
    &request,
    vec![
      old_handoff.clone(),
      handoff,
      decision,
      warning,
      deprecated,
      pinned,
      fact.clone(),
    ],
    vec![
      matched(fact, 2.0, 1.0),
      matched(other_task, 9.0, 9.0),
      matched(proposal, 9.0, 9.0),
      matched(old_handoff, 1.0, 0.0),
    ],
  );
  let expected = [
    (InjectionReason::Handoff, "Built the store."),
    (InjectionReason::Pinned, "The repo uses 2-space indent."),
    (InjectionReason::Warning, "Never VACUUM during a run."),
    (InjectionReason::Decision, "Use FTS5."),
    (InjectionReason::Match, "Tokens are estimated from bytes."),
    (InjectionReason::Match, "Planned the store."),
  ];
  let expected: Vec<(InjectionReason, String)> = expected
    .iter()
    .map(|(reason, text)| (*reason, (*text).to_owned()))
    .collect();
  assert_eq!(chosen, expected);
}

#[test]
fn recall_ranks_matches_by_relevance_importance_recency_role_and_confidence() {
  let fixture = Fixture::new();
  let request = fixture.request(MemoryLimits::DEFAULT);
  let relevant = fixture.by_person(MemoryKind::Fact, "Relevant.", 10);
  let recent = fixture.by_person(MemoryKind::Fact, "Recent.", 29);
  let tagged = Memory::by_person(
    MemoryScope::Task(fixture.task),
    SecretScreen::new(Vec::new())
      .check(content(MemoryKind::Fact, "For reviewers.").with_tags(Tags::new(["code-reviewer"]).expect("tags")))
      .expect("no secrets"),
    at(10 * DAY),
  );
  let plain = fixture.by_person(MemoryKind::Fact, "Plain.", 10);
  let chosen: Vec<String> = reasons(
    &request,
    Vec::new(),
    vec![
      matched(plain, 0.5, 0.5),
      matched(tagged, 0.5, 0.5),
      matched(recent, 0.5, 0.5),
      matched(relevant, 1.0, 1.0),
    ],
  )
  .into_iter()
  .map(|(_, text)| text)
  .collect();
  // Relevance 0.40 beats the role (0.15), which beats recency (0.15 · (1 − 0.5^(20/14)) ≈ 0.09).
  assert_eq!(chosen, ["Relevant.", "For reviewers.", "Recent.", "Plain."]);
}

#[test]
fn recall_caps_at_eight_records_and_three_thousand_tokens() {
  let fixture = Fixture::new();
  let request = fixture.request(MemoryLimits::DEFAULT);
  let many: Vec<MemoryMatch> = (0..12)
    .map(|n| {
      matched(
        fixture.by_person(MemoryKind::Fact, &format!("Fact number {n}."), 1),
        1.0,
        1.0,
      )
    })
    .collect();
  assert_eq!(recall(&request, Vec::new(), many).entries().len(), 8);

  // Just over 1,000 tokens each: a third would pass 3,000, so the small one comes third.
  let big = |n: i64| fixture.by_person(MemoryKind::Fact, &format!("{n}{}", "x".repeat(4_000)), n);
  let small = fixture.by_person(MemoryKind::Fact, "Small.", 0);
  let recalled = recall(
    &request,
    Vec::new(),
    vec![
      matched(big(3), 1.0, 1.0),
      matched(big(2), 0.9, 0.9),
      matched(big(1), 0.8, 0.8),
      matched(small, 0.1, 0.1),
    ],
  );
  let tokens: u32 = recalled.entries().iter().map(|entry| entry.tokens).sum();
  assert_eq!(recalled.entries().len(), 3);
  assert!(tokens <= 3_000, "{tokens}");
  assert_eq!(recalled.entries()[2].memory.content().text(), "Small.");
  assert_eq!(recalled.injected().len(), 3);

  let one = MemoryLimits {
    records: NonZeroU16::MIN,
    tokens: NonZeroU32::new(3_000).expect("above zero"),
  };
  assert_eq!(
    recall(
      &fixture.request(one),
      Vec::new(),
      vec![matched(fixture.by_person(MemoryKind::Fact, "Only.", 1), 1.0, 1.0)]
    )
    .entries()
    .len(),
    1
  );
}

#[test]
fn recall_leaves_out_records_with_a_chosen_key_or_the_same_text() {
  let fixture = Fixture::new();
  let request = fixture.request(MemoryLimits::DEFAULT);
  let keyed = |text: &str, day| {
    Memory::by_person(
      MemoryScope::Task(fixture.task),
      SecretScreen::new(Vec::new())
        .check(content(MemoryKind::Fact, text).with_key(MemoryKey::new("db.engine").expect("key")))
        .expect("no secrets"),
      at(day * DAY),
    )
  };
  let chosen = reasons(
    &request,
    Vec::new(),
    vec![
      matched(keyed("SQLite.", 5), 1.0, 1.0),
      matched(keyed("Postgres.", 1), 0.9, 0.9),
      matched(fixture.by_person(MemoryKind::Fact, "Same  text.", 1), 0.8, 0.8),
      matched(fixture.by_person(MemoryKind::Fact, "same text.", 1), 0.7, 0.7),
    ],
  );
  let texts: Vec<&str> = chosen.iter().map(|(_, text)| text.as_str()).collect();
  assert_eq!(texts, ["SQLite.", "Same  text."]);
}
