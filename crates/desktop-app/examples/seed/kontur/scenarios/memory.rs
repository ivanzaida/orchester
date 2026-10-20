//! `memory`: the `tasks` project with the memory the design's Memory screens show (`Artifacts & Memory` `g5dmWI`,
//! `Project memory` `x4pFb`). Both memories save agents' records after approval. KON-12, whose third run works on
//! Implement again after Review returned the work, has its steps' handoffs (Review's the latest), three decisions, a
//! pinned warning a person wrote and edited, two facts, and two proposals waiting: Builder's replacement of the
//! retry budget decision and a warning. The project has decisions (db.engine superseding an earlier version, pinned
//! and edited by a person), a warning, facts, a to do, a summary and an observation, a deprecated and a rejected
//! fact, and Planner's proposal to replace api.errors; KON-16 has a fact waiting too.

use orchester_core::{
  memory::{
    AgentProvenance, AgentWrite, Confidence, Importance, Memory, MemoryContent, MemoryKey, MemoryKind, MemoryScope,
    MemorySettings, MemoryStore, ScreenedContent, SecretScreen, Tags, WritePolicy,
  },
  run::StepExecutor,
  store::RunStore,
};

use super::tasks;
use crate::kontur::{Kontur, SeedError};

/// Which execution of a step wrote a record: its first attempt's, or the latest.
#[derive(Clone, Copy)]
enum Attempt {
  First,
  Latest,
}

/// Who wrote a record: the agent of `step` of the latest run of task `task`, in `attempt`, `confidence` sure.
#[derive(Clone, Copy)]
struct By {
  task: &'static str,
  step: &'static str,
  attempt: Attempt,
  confidence: Confidence,
}

const fn by(task: &'static str, step: &'static str, attempt: Attempt) -> By {
  By {
    task,
    step,
    attempt,
    confidence: Confidence::High,
  }
}

/// What a record says: its type, key, text, tags and importance.
#[derive(Clone, Copy)]
struct Says {
  kind: MemoryKind,
  key: Option<&'static str>,
  text: &'static str,
  tags: &'static [&'static str],
}

const fn says(kind: MemoryKind, key: Option<&'static str>, text: &'static str, tags: &'static [&'static str]) -> Says {
  Says { kind, key, text, tags }
}

const AFTER_APPROVAL: MemorySettings = MemorySettings {
  task_writes: WritePolicy::AfterApproval,
  project_writes: WritePolicy::AfterApproval,
  limits: orchester_core::memory::MemoryLimits::DEFAULT,
};

const AUTOMATIC: MemorySettings = MemorySettings {
  task_writes: WritePolicy::Automatic,
  project_writes: WritePolicy::Automatic,
  limits: orchester_core::memory::MemoryLimits::DEFAULT,
};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  tasks::project(kontur)?;
  kontur.db.save_memory_settings(&AFTER_APPROVAL)?;
  task_memory(kontur)?;
  project_memory(kontur)?;
  Ok(())
}

/// KON-12's memory (`a0PLO`).
fn task_memory(kontur: &mut Kontur) -> Result<(), SeedError> {
  let task = MemoryScope::Task(kontur.find("KON-12")?.id());
  let research = by("KON-12", "research", Attempt::First);
  let plan = by("KON-12", "plan", Attempt::First);
  let built = by("KON-12", "implement", Attempt::First);
  let building = by("KON-12", "implement", Attempt::Latest);
  let review = by("KON-12", "review", Attempt::First);
  let mut records = vec![
    agent(kontur, (task, research), handoff(RESEARCH_HANDOFF), 300)?,
    agent(kontur, (task, plan), handoff(PLAN_HANDOFF), 290)?,
    agent(kontur, (task, built), handoff(IMPLEMENT_HANDOFF), 260)?,
    agent(kontur, (task, review), handoff(REVIEW_HANDOFF), 200)?,
    agent(kontur, (task, research), DB_ENGINE_FACT, 296)?,
    agent(kontur, (task, building), RETRY_AFTER, 150)?,
    agent(kontur, (task, plan), BACKOFF, 285)?,
    agent(kontur, (task, built), ATTEMPTS_LOG, 262)?,
  ];
  let budget = agent(kontur, (task, plan), RETRY_BUDGET, 288)?;
  let mut warning = person(task, FOUR_XX, kontur.ago(320));
  warning.set_pinned(true, kontur.ago(319)).map_err(invalid)?;
  warning.edit(screened(FOUR_XX)?, kontur.ago(300)).map_err(invalid)?;
  records.push(warning);
  let replacement = proposal(kontur, building, (&budget, RETRY_BUDGET_PER_ENDPOINT), 120)?;
  let fake_clock = proposal_new(kontur, (task, building), FAKE_CLOCK, 110)?;
  records.extend([budget, replacement, fake_clock]);
  kontur.db.add_memories(&records)?;
  Ok(())
}

/// The project's memory (`xY2Uj`) and the proposals of its queue.
fn project_memory(kontur: &mut Kontur) -> Result<(), SeedError> {
  let project = MemoryScope::Project;
  let research = by("KON-12", "research", Attempt::First);
  let plan = by("KON-12", "plan", Attempt::First);
  let built = by("KON-12", "implement", Attempt::First);
  let review = by("KON-12", "review", Attempt::First);
  let api_errors = agent(kontur, (project, review), API_ERRORS, 9 * 24 * 60)?;
  let records = vec![
    agent(kontur, (project, plan), AUTH_TOKENS, 4 * 24 * 60)?,
    agent(kontur, (project, review), TESTS_WEBHOOKS, 3 * 24 * 60)?,
    agent(kontur, (project, research), DB_MIGRATIONS, 6 * 24 * 60)?,
    agent(kontur, (project, research), CI_CHECKS, 9 * 24 * 60)?,
    person(project, REPO_LAYOUT, kontur.ago(20 * 24 * 60)),
    agent(kontur, (project, built), RETRY_HELPER, 100)?,
    agent(kontur, (project, review), OVERVIEW, 3 * 24 * 60)?,
    agent(kontur, (project, built), CI_TIMES, 6 * 24 * 60)?,
  ];
  let mut deprecated = agent(kontur, (project, research), CI_RUNNER, 12 * 24 * 60)?;
  deprecated.deprecate(kontur.ago(5 * 24 * 60)).map_err(invalid)?;
  let mut rejected = proposal_new(kontur, (project, research), GITHUB_AUTH, 8 * 24 * 60)?;
  rejected.reject(kontur.ago(7 * 24 * 60)).map_err(invalid)?;
  let replacement = proposal(kontur, plan, (&api_errors, API_ERRORS_TYPED), 3 * 60)?;
  let kon_16 = MemoryScope::Task(kontur.find("KON-16")?.id());
  let rate_limit = proposal_new(kontur, (kon_16, by("KON-16", "plan", Attempt::Latest)), RATE_LIMIT, 70)?;
  let mut all = records;
  all.extend([deprecated, rejected, api_errors, replacement, rate_limit]);
  kontur.db.add_memories(&all)?;
  db_engine(kontur, (project, research, plan))
}

/// db.engine: an earlier version an agent superseded, then pinned and edited by a person.
fn db_engine(kontur: &mut Kontur, (project, research, plan): (MemoryScope, By, By)) -> Result<(), SeedError> {
  let mut earlier = agent(kontur, (project, research), DB_ENGINE_POSTGRES, 26 * 24 * 60)?;
  kontur.db.add_memories(std::slice::from_ref(&earlier))?;
  let write = write(kontur, plan)?;
  let mut engine = Memory::replacing(
    &earlier,
    write,
    screened(DB_ENGINE_SQLITE)?,
    &AUTOMATIC,
    kontur.ago(25 * 24 * 60),
  )
  .map_err(invalid)?;
  earlier
    .supersede(&mut engine, kontur.ago(25 * 24 * 60))
    .map_err(invalid)?;
  kontur.db.supersede_memory(&mut earlier, &mut engine)?;
  engine.set_pinned(true, kontur.ago(20 * 24 * 60)).map_err(invalid)?;
  engine
    .edit(screened(DB_ENGINE_SQLITE)?, kontur.ago(18 * 24 * 60))
    .map_err(invalid)?;
  kontur.db.save_memory(&mut engine)?;
  Ok(())
}

/// An agent's active record, written `minutes` ago.
fn agent(kontur: &Kontur, (scope, by): (MemoryScope, By), record: Says, minutes: i64) -> Result<Memory, SeedError> {
  Ok(Memory::by_agent(
    scope,
    write(kontur, by)?,
    screened(record)?,
    &AUTOMATIC,
    kontur.ago(minutes),
  ))
}

/// An agent's proposal that replaces `old`, of its scope, written `minutes` ago from three transcript entries.
fn proposal(kontur: &Kontur, by: By, (old, record): (&Memory, Says), minutes: i64) -> Result<Memory, SeedError> {
  let mut write = write(kontur, by)?;
  write.sources = vec![3, 4, 5];
  Memory::replacing(old, write, screened(record)?, &AFTER_APPROVAL, kontur.ago(minutes)).map_err(invalid)
}

/// An agent's new proposal, written `minutes` ago.
fn proposal_new(
  kontur: &Kontur,
  (scope, by): (MemoryScope, By),
  record: Says,
  minutes: i64,
) -> Result<Memory, SeedError> {
  let mut write = write(kontur, by)?;
  write.confidence = Confidence::Medium;
  Ok(Memory::by_agent(
    scope,
    write,
    screened(record)?,
    &AFTER_APPROVAL,
    kontur.ago(minutes),
  ))
}

/// A person's record of `scope`, written at `at`.
fn person(scope: MemoryScope, record: Says, at: orchester_core::time::Timestamp) -> Memory {
  let content = screened(record).expect("the seed's records hold no secret");
  Memory::by_person(scope, content, at)
}

/// The agent of `by`'s step: its profile, run and execution.
fn write(kontur: &Kontur, by: By) -> Result<AgentWrite, SeedError> {
  let task = kontur.find(by.task)?;
  let run = kontur
    .db
    .runs_of_task(task.id())?
    .pop()
    .ok_or_else(|| SeedError::Invalid(format!("{} has no run", by.task)))?
    .map_err(|unreadable| SeedError::Invalid(format!("{}'s run cannot be read: {unreadable:?}", by.task)))?;
  let attempts = run.attempts();
  let attempt = match by.attempt {
    Attempt::First => attempts.first(),
    Attempt::Latest => attempts.iter().rev().find(|attempt| {
      attempt
        .executions()
        .iter()
        .any(|execution| execution.step().as_str() == by.step)
    }),
  };
  let execution = attempt
    .and_then(|attempt| {
      attempt
        .executions()
        .iter()
        .find(|execution| execution.step().as_str() == by.step)
    })
    .ok_or_else(|| SeedError::NoSuchStep(by.step.to_owned()))?;
  let Some(StepExecutor::Agent { profile, .. }) = run.snapshot().step(execution.step()).map(|step| step.executor())
  else {
    return Err(SeedError::Invalid(format!(
      "{}'s step {} is no agent's",
      by.task, by.step
    )));
  };
  Ok(AgentWrite {
    provenance: AgentProvenance {
      profile: profile.resource,
      run: run.id(),
      step: execution.step().clone(),
      execution: execution.id(),
    },
    confidence: by.confidence,
    sources: Vec::new(),
  })
}

fn screened(record: Says) -> Result<ScreenedContent, SeedError> {
  let mut content = MemoryContent::new(record.kind, record.text).map_err(invalid)?;
  if let Some(key) = record.key {
    content = content.with_key(MemoryKey::new(key).map_err(invalid)?);
  }
  let tags = Tags::new(record.tags.iter().copied()).map_err(invalid)?;
  let importance = if record.kind == MemoryKind::Handoff {
    Importance::High
  } else {
    Importance::Normal
  };
  SecretScreen::new(Vec::new())
    .check(content.with_tags(tags).with_importance(importance))
    .map_err(invalid)
}

fn invalid(error: impl std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}

const fn handoff(text: &'static str) -> Says {
  says(MemoryKind::Handoff, None, text, &[])
}

const RESEARCH_HANDOFF: &str = "The sender retries in a fixed loop of 5 in crates/webhooks/src/sender.rs.\n\nCompleted:\n- Read the \
                                sender and its tests";
const PLAN_HANDOFF: &str = "Planned exponential backoff with a per-endpoint retry budget.\n\nCompleted:\n- Wrote the plan \
                            and had it approved";
const IMPLEMENT_HANDOFF: &str = "Added backoff with full jitter and logged every attempt.\n\nCompleted:\n- Backoff in \
                                 sender.rs\n- webhook_attempts table";
const REVIEW_HANDOFF: &str = "Returned the work to Implement: two of five criteria are not met.\n\nCompleted:\n- \
                              Checked the 5 criteria against a91f3e0; ran cargo test -p webhooks.\n\nOpen questions:\n- Should \
                              503 honor Retry-After as well as 429?\n\nThe next agent should know:\n- 429 responses ignore \
                              Retry-After, and no test pins the jitter bound.";
const RETRY_BUDGET: Says = says(
  MemoryKind::Decision,
  Some("webhooks.retry.budget"),
  "Retry budget is a global constant of 5 attempts (MAX_RETRIES).",
  &["webhooks"],
);
const RETRY_BUDGET_PER_ENDPOINT: Says = says(
  MemoryKind::Decision,
  Some("webhooks.retry.budget"),
  "Retry budget is per endpoint: EndpointSettings.retry_budget, default 5 attempts. The global MAX_RETRIES constant \
   goes away.",
  &["webhooks", "settings"],
);
const BACKOFF: Says = says(
  MemoryKind::Decision,
  Some("webhooks.backoff"),
  "Exponential backoff from 500 ms, capped at 60 s, with full jitter.",
  &["webhooks", "retry"],
);
const ATTEMPTS_LOG: Says = says(
  MemoryKind::Decision,
  Some("webhooks.attempts.log"),
  "Every attempt is recorded in webhook_attempts with status, delay and delay_source.",
  &[],
);
const FOUR_XX: Says = says(
  MemoryKind::Warning,
  None,
  "Don't retry 4xx other than 408 and 429: endpoints treat repeated 4xx as abuse and block the sender.",
  &["webhooks", "rate-limit"],
);
const DB_ENGINE_FACT: Says = says(
  MemoryKind::Fact,
  Some("webhooks.store"),
  "Webhook attempts live in the project's SQLite database; migrations are in migrations/ and run with sqlx.",
  &["database"],
);
const RETRY_AFTER: Says = says(
  MemoryKind::Fact,
  None,
  "Retry-After arrives as seconds or as an HTTP date; the httpdate crate parses both.",
  &["http"],
);
const FAKE_CLOCK: Says = says(
  MemoryKind::Warning,
  None,
  "Retry tests use the fake clock in tests/support/clock.rs; a real sleep in sender.rs makes them hang on CI.",
  &["tests", "webhooks"],
);
const DB_ENGINE_POSTGRES: Says = says(
  MemoryKind::Decision,
  Some("db.engine"),
  "Kontur uses Postgres 16 from docker-compose.yml for local development.",
  &["storage"],
);
const DB_ENGINE_SQLITE: Says = says(
  MemoryKind::Decision,
  Some("db.engine"),
  "Kontur keeps its data in SQLite through sqlx. No Postgres until there is a server; docker-compose.yml is gone.",
  &["storage", "sqlite"],
);
const API_ERRORS: Says = says(
  MemoryKind::Decision,
  Some("api.errors"),
  "Handlers map domain errors to HTTP statuses in one place, error_response(); a handler never builds an error \
   response itself.",
  &["api", "errors"],
);
const API_ERRORS_TYPED: Says = says(
  MemoryKind::Decision,
  Some("api.errors"),
  "API handlers return typed errors as { code, message } with the status taken from the error kind; no handler \
   builds an error response itself.",
  &["api", "errors"],
);
const AUTH_TOKENS: Says = says(
  MemoryKind::Decision,
  Some("auth.tokens"),
  "GitHub tokens are read from gh when a request needs one and are never stored in the database.",
  &["github", "auth"],
);
const TESTS_WEBHOOKS: Says = says(
  MemoryKind::Warning,
  Some("tests.webhooks"),
  "Webhook delivery tests are flaky on Windows: the mock server's port is reused before it closes. Rerun with \
   --test-threads=1 before treating a failure as real.",
  &["webhooks", "tests"],
);
const DB_MIGRATIONS: Says = says(
  MemoryKind::Fact,
  Some("db.migrations"),
  "Migrations live in crates/store/migrations and run at startup; every migration needs a matching down file.",
  &["storage"],
);
const CI_CHECKS: Says = says(
  MemoryKind::Fact,
  Some("ci.checks"),
  "CI runs cargo fmt, clippy with warnings denied and the tests on Windows and macOS; a pull request merges only when \
   all pass.",
  &["ci"],
);
const REPO_LAYOUT: Says = says(
  MemoryKind::Fact,
  Some("repo.layout"),
  "Domain code lives in crates/core and HTTP handlers in crates/api; handlers stay thin.",
  &["architecture"],
);
const RETRY_HELPER: Says = says(
  MemoryKind::Todo,
  None,
  "Remove the old retry helper in crates/api/src/retry.rs once KON-12 is merged.",
  &["webhooks"],
);
const OVERVIEW: Says = says(
  MemoryKind::Summary,
  Some("webhooks.overview"),
  "Webhook work so far: KON-12 adds retries to the sender, KON-29 signing secrets per endpoint, KON-30 signature \
   checks on receive. Shared code lives in crates/webhooks.",
  &["webhooks"],
);
const CI_TIMES: Says = says(
  MemoryKind::Observation,
  None,
  "macOS CI builds take about 9 minutes and Windows about 6; the cache is cold after every Cargo.lock change.",
  &["ci"],
);
const CI_RUNNER: Says = says(
  MemoryKind::Fact,
  Some("ci.runner"),
  "CI runs on the self-hosted Windows runner kontur-ci-01; macOS jobs wait for it.",
  &["ci"],
);
const GITHUB_AUTH: Says = says(
  MemoryKind::Fact,
  Some("github.auth"),
  "Kontur authenticates to GitHub with a GitHub App installation token.",
  &["github"],
);
const RATE_LIMIT: Says = says(
  MemoryKind::Fact,
  Some("github.rate_limit"),
  "GitHub's secondary rate limit answers 403 with Retry-After, not 429; treat both as rate limited.",
  &["github"],
);
