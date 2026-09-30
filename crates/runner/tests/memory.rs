//! Task memory across a run's steps: a finished step's handoff is kept as task memory and reaches
//! the next step's prompt, as the project's Memory setting says, and what each invocation was
//! given is recorded with it. Against `orchester-fake-codex` and `orchester-fake-claude` ("Leave a
//! handoff"); needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test memory memory`.

mod support;

use std::num::{NonZeroU16, NonZeroU32};

use orchester_core::{
  connection::{
    AgentProvider,
    stream::{AgentEvent, RunnerProblem},
  },
  id::{IdempotencyKey, RunId, SecretId, TaskId},
  memory::{
    InjectionReason, MemoryAuthor, MemoryFilter, MemoryKind, MemoryLimits, MemorySettings, MemoryStatus, MemoryStore,
    ScopeFilter, WritePolicy,
  },
  run::RunStatus,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use self::support::{Setup, TestProject, project, scheduler_with, wait_for};

const BRIEF: &str = "Leave a handoff.";

fn implement_then_review(provider: AgentProvider) -> Setup {
  Setup {
    review: true,
    provider,
    ..Setup::new(BRIEF)
  }
}

async fn run_to_the_end(project: &TestProject) -> RunId {
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let ended = wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
  assert_eq!(ended.status(), &RunStatus::Succeeded);
  run
}

/// The prompt each invocation's CLI read, in launch order.
fn prompts(db: &ProjectDb, run: RunId) -> Vec<String> {
  db.invocations(run)
    .expect("invocations")
    .iter()
    .map(|invocation| {
      db.activity(run, 0)
        .expect("activity")
        .into_iter()
        .filter(|entry| entry.invocation == invocation.id)
        .filter_map(|entry| match entry.event {
          AgentEvent::Raw { line, .. } => line.strip_prefix("prompt| ").map(str::to_owned),
          _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
    })
    .collect()
}

fn task_memory(db: &ProjectDb, task: TaskId) -> Vec<(MemoryKind, MemoryStatus, String)> {
  let filter = MemoryFilter {
    scope: ScopeFilter::Task(task),
    status: None,
    pinned_only: false,
  };
  db.memories(&filter)
    .expect("memories")
    .into_iter()
    .map(|memory| {
      (
        memory.content().kind(),
        memory.status(),
        memory.content().text().to_owned(),
      )
    })
    .collect()
}

fn injected(db: &ProjectDb, run: RunId) -> Vec<Vec<InjectionReason>> {
  db.invocations(run)
    .expect("invocations")
    .iter()
    .map(|invocation| {
      db.injected_memory(invocation.id)
        .expect("injected")
        .into_iter()
        .map(|entry| entry.reason)
        .collect()
    })
    .collect()
}

async fn handoff_reaches_the_next_step(provider: AgentProvider) {
  let project = project(&implement_then_review(provider));
  let run = run_to_the_end(&project).await;
  let db = ProjectDb::open(project.repo.path()).expect("open");

  let memory = task_memory(&db, project.task);
  let kinds: Vec<(MemoryKind, MemoryStatus)> = memory.iter().map(|(kind, status, _)| (*kind, *status)).collect();
  assert_eq!(
    kinds,
    [
      (MemoryKind::Handoff, MemoryStatus::Active),
      (MemoryKind::Decision, MemoryStatus::Active),
      (MemoryKind::Handoff, MemoryStatus::Active),
      (MemoryKind::Decision, MemoryStatus::Active),
    ],
    "{memory:?}"
  );
  assert!(
    memory[0]
      .2
      .starts_with("Finished Implement\n\nCompleted:\n- Appended hello"),
    "{memory:?}"
  );
  let stored = db
    .memories(&MemoryFilter {
      scope: ScopeFilter::Task(project.task),
      status: None,
      pinned_only: false,
    })
    .expect("memories");
  let MemoryAuthor::Agent(agent) = stored[0].author() else {
    panic!("an agent wrote the handoff");
  };
  assert_eq!((agent.run, agent.step.as_str()), (run, "implement"));
  assert_eq!(stored[0].sources().len(), 1, "the final message's transcript entry");

  let prompts = prompts(&db, run);
  assert!(!prompts[0].contains("# Task memory"), "{}", prompts[0]);
  let review = &prompts[1];
  assert!(review.contains("# Task memory"), "{review}");
  assert!(
    review.contains("## Latest handoff, from step `implement`\nFinished Implement"),
    "{review}"
  );
  assert!(
    review.contains("## Decisions and warnings\n- Decision: Greet in README.md from Implement"),
    "{review}"
  );
  assert!(
    review.find("# Task memory").expect("memory") < review.find("# Task ORC-1:").expect("task"),
    "{review}"
  );
  assert_eq!(
    injected(&db, run),
    [vec![], vec![InjectionReason::Handoff, InjectionReason::Decision]]
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn memory_handoff_of_codex_reaches_the_next_step() {
  handoff_reaches_the_next_step(AgentProvider::CodexCli).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn memory_handoff_of_claude_reaches_the_next_step() {
  handoff_reaches_the_next_step(AgentProvider::ClaudeCodeCli).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn memory_saved_after_approval_is_proposed_and_never_injected() {
  let project = project(&implement_then_review(AgentProvider::CodexCli));
  let settings = MemorySettings {
    task_writes: WritePolicy::AfterApproval,
    ..MemorySettings::DEFAULT
  };
  ProjectDb::open(project.repo.path())
    .expect("open")
    .save_memory_settings(&settings)
    .expect("save the setting");
  let run = run_to_the_end(&project).await;
  let db = ProjectDb::open(project.repo.path()).expect("open");
  let memory = task_memory(&db, project.task);
  assert_eq!(memory.len(), 4, "{memory:?}");
  assert!(
    memory.iter().all(|(_, status, _)| *status == MemoryStatus::Proposed),
    "{memory:?}"
  );
  assert!(!prompts(&db, run)[1].contains("# Task memory"));
  assert_eq!(injected(&db, run), [vec![], vec![]]);
}

#[tokio::test(flavor = "multi_thread")]
async fn memory_limits_come_from_the_setting() {
  let project = project(&implement_then_review(AgentProvider::CodexCli));
  let settings = MemorySettings {
    limits: MemoryLimits {
      records: NonZeroU16::MIN,
      tokens: NonZeroU32::new(3_000).expect("above zero"),
    },
    ..MemorySettings::DEFAULT
  };
  ProjectDb::open(project.repo.path())
    .expect("open")
    .save_memory_settings(&settings)
    .expect("save the setting");
  let run = run_to_the_end(&project).await;
  let db = ProjectDb::open(project.repo.path()).expect("open");
  assert_eq!(injected(&db, run), [vec![], vec![InjectionReason::Handoff]]);
  assert!(!prompts(&db, run)[1].contains("- Decision:"));
}

#[tokio::test(flavor = "multi_thread")]
async fn memory_refuses_a_handoff_holding_a_secret_value() {
  let value = "Greet in README.md";
  let secret = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("GREETING_TOKEN").expect("valid name"),
      kind: SecretKind::AccessToken,
      environment: Environment::Development,
    },
    Timestamp::now(),
  );
  let project = project(&Setup {
    secrets: vec![(secret, value)],
    ..implement_then_review(AgentProvider::CodexCli)
  });
  let run = run_to_the_end(&project).await;
  let db = ProjectDb::open(project.repo.path()).expect("open");
  assert!(task_memory(&db, project.task).is_empty());
  let details: Vec<String> = db
    .activity(run, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::RunnerProblem {
        problem: RunnerProblem::MemoryNotSaved { detail },
      } => Some(detail),
      _ => None,
    })
    .collect();
  assert_eq!(details.len(), 2, "one per step: {details:?}");
  assert!(
    details.iter().all(|detail| detail.contains("GREETING_TOKEN")),
    "{details:?}"
  );
  assert!(!details.iter().any(|detail| detail.contains(value)), "{details:?}");
}
