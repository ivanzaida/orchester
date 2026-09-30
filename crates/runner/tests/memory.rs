//! Task memory across a run's steps: a finished step's handoff is kept as task memory and reaches
//! the next step's prompt, as the project's Memory setting says, and what each invocation was
//! given is recorded with it. Against `orchester-fake-codex` and `orchester-fake-claude` ("Leave a
//! handoff"); needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test memory memory`.

mod support;

use orchester_core::{
  connection::{
    AgentProvider,
    stream::{AgentEvent, MemoryNotSavedReason, RunnerProblem},
  },
  id::{IdempotencyKey, RunId, SecretId, TaskId},
  memory::{
    Delivery, InjectionReason, Memory, MemoryAuthor, MemoryContent, MemoryFilter, MemoryKind, MemoryLimits,
    MemoryScope, MemorySettings, MemoryStatus, MemoryStore, ScopeFilter, SecretScreen, WritePolicy,
  },
  run::{PlanDecision, Run, RunStatus},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::{ActivityStore, RunStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use self::support::{Setup, StepSpec, TestProject, project, scheduler_with, wait_for, wait_for_idle};

const BRIEF: &str = "Leave a handoff.";

fn implement_then_review(provider: AgentProvider) -> Setup {
  Setup {
    review: true,
    provider,
    ..Setup::new(BRIEF)
  }
}

async fn run_to_the_end(project: &TestProject) -> RunId {
  let (run, status) = run_until_it_ends(project).await;
  assert_eq!(status, RunStatus::Succeeded);
  run
}

async fn run_until_it_ends(project: &TestProject) -> (RunId, RunStatus) {
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let ended = wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
  // The last step's handoff is kept once the run has ended.
  wait_for_idle(&scheduler).await;
  (run, ended.status().clone())
}

fn graph(steps: Vec<StepSpec>) -> TestProject {
  project(&Setup {
    steps,
    ..Setup::new(BRIEF)
  })
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

/// Why each record an invocation's prompt was given was chosen, per invocation.
fn injected(db: &ProjectDb, run: RunId) -> Vec<Vec<InjectionReason>> {
  recorded(db, run)
    .into_iter()
    .map(|entries| {
      entries
        .into_iter()
        .filter(|(_, delivery)| delivery.is_given())
        .map(|(reason, _)| reason)
        .collect()
    })
    .collect()
}

/// Every record chosen for each invocation's prompt and what became of it.
fn recorded(db: &ProjectDb, run: RunId) -> Vec<Vec<(InjectionReason, Delivery)>> {
  db.invocations(run)
    .expect("invocations")
    .iter()
    .map(|invocation| {
      db.injected_memory(invocation.id)
        .expect("injected")
        .into_iter()
        .map(|entry| (entry.reason, entry.delivery))
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
    // Implement's summary is its result among the results of earlier steps, so it is not repeated.
    review.contains(
      "## Latest handoff, from step `implement`\n\n> Its summary is the step's result under \"Results of earlier \
       steps\".\n>\n> Completed:\n> - Appended hello\n>\n> The next agent should know:"
    ),
    "{review}"
  );
  assert!(
    review.contains("## Decisions and warnings\n\nDecision:\n> Greet in README.md from Implement"),
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
  // Saving automatically from now on leaves what was proposed waiting for a person.
  let mut db = db;
  db.save_memory_settings(&MemorySettings::DEFAULT)
    .expect("save the setting");
  let memory = task_memory(&db, project.task);
  assert!(
    memory.iter().all(|(_, status, _)| *status == MemoryStatus::Proposed),
    "{memory:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn memory_limits_come_from_the_setting() {
  let project = project(&implement_then_review(AgentProvider::CodexCli));
  let settings = MemorySettings {
    limits: MemoryLimits::new(1, 3_000).expect("valid limits"),
    ..MemorySettings::DEFAULT
  };
  ProjectDb::open(project.repo.path())
    .expect("open")
    .save_memory_settings(&settings)
    .expect("save the setting");
  let run = run_to_the_end(&project).await;
  let db = ProjectDb::open(project.repo.path()).expect("open");
  assert_eq!(
    recorded(&db, run),
    [
      vec![],
      vec![
        (InjectionReason::Handoff, Delivery::Given),
        (InjectionReason::Decision, Delivery::OverRecordLimit)
      ]
    ]
  );
  assert!(!prompts(&db, run)[1].contains("Decision:\n>"));
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
  let reasons: Vec<MemoryNotSavedReason> = problems(&db, run)
    .into_iter()
    .filter_map(|problem| match problem {
      RunnerProblem::MemoryNotSaved { reason } => Some(reason),
      _ => None,
    })
    .collect();
  let refused = MemoryNotSavedReason::ContainsSecret {
    secret: "GREETING_TOKEN".to_owned(),
  };
  assert_eq!(reasons, [refused.clone(), refused], "one per step");
  assert!(!format!("{reasons:?}").contains(value));
}

fn problems(db: &ProjectDb, run: RunId) -> Vec<RunnerProblem> {
  db.activity(run, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::RunnerProblem { problem } => Some(problem),
      _ => None,
    })
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn memory_an_unreadable_record_is_reported_once_and_left_out() {
  let project = project(&implement_then_review(AgentProvider::CodexCli));
  let root = project.repo.path();
  {
    let mut db = ProjectDb::open(root).expect("open");
    let content = SecretScreen::new(Vec::new())
      .check(MemoryContent::new(MemoryKind::Fact, "Readable.").expect("content"))
      .expect("no secrets");
    let broken = Memory::by_person(MemoryScope::Project, content, Timestamp::now());
    db.add_memories(std::slice::from_ref(&broken)).expect("add");
    // A key over two lines, which no record can have: the row was edited outside Orchester.
    let raw = rusqlite::Connection::open(root.join(".orchester").join("orchester.db")).expect("open");
    raw
      .execute(
        "UPDATE memories SET key = 'two' || char(10) || 'lines' WHERE id = ?1",
        [broken.id().to_string()],
      )
      .expect("damage the row");
  }
  let run = run_to_the_end(&project).await;
  let db = ProjectDb::open(root).expect("open");
  let unreadable: Vec<RunnerProblem> = problems(&db, run)
    .into_iter()
    .filter(|problem| matches!(problem, RunnerProblem::MemoryUnreadable { .. }))
    .collect();
  assert_eq!(unreadable, [RunnerProblem::MemoryUnreadable { records: 1 }]);
  assert_eq!(db.unreadable_memories().expect("listed").len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn memory_a_failed_step_leaves_a_failed_attempt_without_decisions() {
  let project = graph(vec![StepSpec::new("implement", "Implement", "Report failure.", &[])]);
  let (_, status) = run_until_it_ends(&project).await;
  assert!(matches!(status, RunStatus::Failed(_)), "{status:?}");
  let db = ProjectDb::open(project.repo.path()).expect("open");
  let memory = task_memory(&db, project.task);
  assert_eq!(memory.len(), 1, "{memory:?}");
  assert_eq!((memory[0].0, memory[0].1), (MemoryKind::Handoff, MemoryStatus::Active));
  assert!(memory[0].2.starts_with("This attempt failed;"), "{memory:?}");
  assert!(!memory[0].2.contains("Decisions:"), "{memory:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn memory_retries_keep_one_handoff_and_the_current_decisions_per_step() {
  let project = graph(vec![
    StepSpec::new("implement", "Implement", "Sign the line.", &[]),
    StepSpec::new("review", "Review", "Fail once.", &["implement"]).retry("implement", 2),
  ]);
  run_to_the_end(&project).await;
  let db = ProjectDb::open(project.repo.path()).expect("open");
  let memory = task_memory(&db, project.task);
  let mut active: Vec<(MemoryKind, String)> = memory
    .iter()
    .filter(|(_, status, _)| *status == MemoryStatus::Active)
    .map(|(kind, _, text)| (*kind, text.lines().next().unwrap_or_default().to_owned()))
    .collect();
  active.sort();
  assert_eq!(
    active,
    [
      (MemoryKind::Decision, "Greet in README.md from Implement".to_owned()),
      (MemoryKind::Decision, "Greet in README.md from Review".to_owned()),
      (MemoryKind::Handoff, "Finished Implement".to_owned()),
      (MemoryKind::Handoff, "Finished Review".to_owned()),
    ],
    "{memory:?}"
  );
  assert!(
    memory.len() > active.len(),
    "the earlier executions' records are kept, replaced"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn memory_an_approved_plan_keeps_its_handoff_only_then() {
  let project = graph(vec![
    StepSpec::new("plan", "Plan", "Propose a plan.", &[]).gate(),
    StepSpec::new("implement", "Implement", "Follow the plan.", &["plan"]),
  ]);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, run, Run::waits_for_a_person).await;
  let db = ProjectDb::open(root).expect("open");
  assert!(
    task_memory(&db, project.task).is_empty(),
    "a proposed plan keeps nothing"
  );
  let plan = db.plans(run).expect("plans").pop().expect("a plan");
  scheduler
    .decide_plan(run, plan.execution, plan.version, PlanDecision::Approved)
    .await
    .expect("approved");
  wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
  wait_for_idle(&scheduler).await;
  let memory = task_memory(&db, project.task);
  assert!(
    memory.iter().any(|(kind, status, text)| *kind == MemoryKind::Handoff
      && *status == MemoryStatus::Active
      && text.starts_with("Finished Plan")),
    "{memory:?}"
  );
  assert!(prompts(&db, run)[1].contains("## Latest handoff, from step `plan`"));
}
