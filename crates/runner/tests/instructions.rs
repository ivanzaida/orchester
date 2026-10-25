//! Project instructions in the prompts agents receive: those injected at start in a new session's
//! prompt on standard input, those injected at resume when a session continues, each invocation
//! with the sources of its prompt, the versions the run pinned when it started, and the run's
//! revisions its prompts name. Against
//! `orchester-fake-codex`; needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test instructions`.

mod support;

use std::{collections::BTreeSet, num::NonZeroU32, path::Path};

use orchester_core::{
  connection::stream::{AgentEvent, InvocationOutcome, RunnerProblem},
  id::{IdempotencyKey, ResourceId},
  name::Name,
  resource::{
    AgentTargets, Deliverable, Draft, InjectionEvent, Instruction, InstructionScope, PromptText, Slug, StepKey,
  },
  run::{PromptPart, PromptSource, RunStatus, StepFailure, StepStatus, Stuck},
  store::{ActivityStore, ResourceStore},
  task::organization::ChangeActor,
  time::Timestamp,
};
use orchester_ipc::AttachSession;
use orchester_store::ProjectDb;

use self::support::{Setup, StepSpec, git, killed_mid_run, project, reported, scheduler, settled, wait_for};

fn rule(text: &str, targets: AgentTargets, events: &[InjectionEvent]) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("valid prompt"),
    targets,
    events.iter().copied().collect(),
    InstructionScope::Project,
  )
  .expect("valid instruction")
}

/// Creates and publishes an instruction; its id.
fn publish(root: &Path, slug: &str, name: &str, body: &Instruction) -> ResourceId {
  let mut db = ProjectDb::open(root).expect("open");
  let id = db
    .create_resource(
      Slug::new(slug).expect("valid slug"),
      Name::new(name).expect("valid name"),
      body,
      Timestamp::now(),
    )
    .expect("create")
    .id();
  db.publish_draft::<Instruction>(id, Timestamp::now()).expect("publish");
  id
}

/// The prompt lines the fake CLI read, per invocation, joined.
fn prompts(db: &ProjectDb, run: orchester_core::id::RunId) -> String {
  reported(db, run, "prompt| ").join("\n")
}

#[tokio::test(flavor = "multi_thread")]
async fn instructions_for_the_agent_at_start_are_in_the_prompt_it_reads() {
  let project = project(&Setup::new("Say hello."));
  let root = project.repo.path();
  let reviewer = AgentTargets::Roles(BTreeSet::from([Name::new("Reviewer").expect("valid role")]));
  let git = publish(
    root,
    "git-conventions",
    "Git conventions",
    &rule(
      "Name commits after {{task.id}}.",
      AgentTargets::All,
      &[InjectionEvent::Start],
    ),
  );
  publish(
    root,
    "review-checklist",
    "Review checklist",
    &rule("Check the tests.", reviewer, &[InjectionEvent::Start]),
  );
  publish(
    root,
    "resume-checks",
    "Resume checks",
    &rule("Run git status first.", AgentTargets::All, &[InjectionEvent::Resume]),
  );
  let (scheduler, mut events) = scheduler(root);
  let run = scheduler
    .start_run(orchester_core::id::IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Succeeded).await;

  let db = ProjectDb::open(root).expect("open");
  let prompt = prompts(&db, run);
  assert!(prompt.contains("# Project rules"), "{prompt}");
  assert!(prompt.contains("## Git conventions"), "{prompt}");
  assert!(prompt.contains("Name commits after ORC-1."), "{prompt}");
  assert!(!prompt.contains("Check the tests."), "{prompt}");
  assert!(!prompt.contains("Run git status first."), "{prompt}");
  let rules = prompt.find("# Project rules").expect("rules");
  assert!(rules < prompt.find("# Current step:").expect("step"), "{prompt}");

  let invocation = db.invocations(run).expect("invocations").remove(0);
  let stored = db.invocation_prompt(invocation.id).expect("read").expect("stored");
  assert_eq!(stored.event, InjectionEvent::Start);
  let instructions: Vec<(ResourceId, PromptPart)> = stored
    .sources
    .iter()
    .filter_map(|source| match source {
      PromptSource::Instruction { resource, part, .. } => Some((*resource, *part)),
      _ => None,
    })
    .collect();
  assert_eq!(instructions, [(git, PromptPart::ProjectRules)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn instructions_for_resuming_reach_a_continued_session_in_their_pinned_version() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let resume = publish(
    root,
    "resume-checks",
    "Resume checks",
    &rule("Run git status first.", AgentTargets::All, &[InjectionEvent::Resume]),
  );
  let run = killed_mid_run(root, project.task).await;
  // Published after the run started: the run keeps v1.
  {
    let mut db = ProjectDb::open(root).expect("open");
    let newer = rule("Something else.", AgentTargets::All, &[InjectionEvent::Resume]);
    db.save_draft(
      resume,
      &Draft::new(
        newer,
        NonZeroU32::new(1).map(orchester_core::resource::VersionNumber::new),
        Timestamp::now(),
      ),
    )
    .expect("draft");
    db.publish_draft::<Instruction>(resume, Timestamp::now())
      .expect("publish v2");
  }
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .attach(AttachSession::Reconnect, NonZeroU32::new(2).expect("above zero"))
    .await
    .expect("attached");
  wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Succeeded).await;

  let db = ProjectDb::open(root).expect("open");
  let invocations = db.invocations(run).expect("invocations");
  let [first, continued] = invocations.as_slice() else {
    panic!("two launches: {invocations:?}");
  };
  let first = db.invocation_prompt(first.id).expect("read").expect("stored");
  assert_eq!(first.event, InjectionEvent::Start);
  assert!(
    first
      .sources
      .iter()
      .all(|source| !matches!(source, PromptSource::Instruction { .. }))
  );
  let continued = db.invocation_prompt(continued.id).expect("read").expect("stored");
  assert_eq!(continued.event, InjectionEvent::Resume);
  assert!(matches!(
    continued.sources.as_slice(),
    [PromptSource::Instruction { resource, version, .. }]
      if *resource == resume
        && *version == orchester_core::run::InstructionVersion::Published { number: orchester_core::resource::VersionNumber::FIRST }
  ));
  let prompt = prompts(&db, run);
  assert!(
    prompt.contains("You were stopped before you finished this step."),
    "{prompt}"
  );
  assert!(prompt.contains("## Resume checks"), "{prompt}");
  assert!(prompt.contains("Run git status first."), "{prompt}");
  assert!(!prompt.contains("Something else."), "{prompt}");
}

#[tokio::test(flavor = "multi_thread")]
async fn instructions_run_revision_names_the_checkpoint_a_step_starts_after() {
  let mut setup = Setup::new("Say hello.");
  setup.steps = vec![
    StepSpec::new("first", "First", "Start from {{run.base_revision}}.", &[]),
    StepSpec::new("second", "Second", "Look at {{run.revision}}.", &["first"]),
  ];
  let project = project(&setup);
  let root = project.repo.path();
  let base = git(root, &["rev-parse", "HEAD"]);
  let (scheduler, mut events) = scheduler(root);
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let finished = wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Succeeded).await;

  // Each step committed a checkpoint on the run's branch: the first's lies below the second's.
  let branch = finished.branch().as_str();
  let first = git(root, &["rev-parse", &format!("{branch}~1")]);
  assert_eq!(
    git(root, &["log", "-1", "--format=%s", &first]),
    "ORC-1: First (run #1)"
  );
  let last = git(root, &["rev-parse", branch]);
  assert_eq!(
    finished
      .last_checkpoint()
      .map(|checkpoint| checkpoint.revision().as_str()),
    Some(last.as_str())
  );
  let prompt = prompts(&ProjectDb::open(root).expect("open"), run);
  assert!(prompt.contains(&format!("Start from {base}.")), "{prompt}");
  assert!(prompt.contains(&format!("Look at {first}.")), "{prompt}");
}

#[tokio::test(flavor = "multi_thread")]
async fn instructions_run_revision_before_the_first_checkpoint_fails_the_step_and_names_it() {
  let mut setup = Setup::new("Say hello.");
  setup.steps = vec![StepSpec::new("first", "First", "Look at {{run.revision}}.", &[])];
  let project = project(&setup);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let finished = wait_for(root, &mut events, run, settled).await;

  let first = StepKey::new("first").expect("key");
  assert_eq!(
    finished.status(),
    &RunStatus::WaitingForPerson(Stuck::BrokeAgain(first.clone()))
  );
  let execution = finished.latest_execution(&first).expect("the step has an execution");
  assert_eq!(execution.status(), StepStatus::Failed);
  assert_eq!(execution.failure(), Some(StepFailure::NotStarted));
  assert!(finished.last_checkpoint().is_none());
  let db = ProjectDb::open(root).expect("open");
  let invocations = db.invocations(run).expect("invocations");
  // It could not start, and neither could its one restart.
  assert_eq!(invocations.len(), 2);
  assert_eq!(invocations[0].outcome, Some(InvocationOutcome::NotStarted));
  // The CLI never started; the step's activity says which variable had no value.
  assert!(reported(&db, run, "argv: ").is_empty());
  let problem = AgentEvent::RunnerProblem {
    problem: RunnerProblem::UnavailableVariable {
      variable: "run.revision".into(),
    },
  };
  assert!(
    db.activity(run, 0)
      .expect("activity")
      .iter()
      .any(|entry| entry.event == problem)
  );
}

/// A run of "Keep working." killed while its agent works, with `then` done to its worktree before it
/// continues; the prompt of the continued session.
async fn resumed_prompt(then: impl FnOnce(&Path)) -> (String, String) {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  publish(
    root,
    "compare",
    "Compare",
    &rule(
      "Compare {{run.revision}}.",
      AgentTargets::All,
      &[InjectionEvent::Resume],
    ),
  );
  let base = git(root, &["rev-parse", "HEAD"]);
  let run = killed_mid_run(root, project.task).await;
  let worktree = root.join(".orchester/worktrees").join(run.to_string());
  then(&worktree);
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .attach(AttachSession::Reconnect, NonZeroU32::new(2).expect("above zero"))
    .await
    .expect("attached");
  wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Succeeded).await;
  let db = ProjectDb::open(root).expect("open");
  let invocations = db.invocations(run).expect("invocations");
  let [_, continued] = invocations.as_slice() else {
    panic!("two launches: {invocations:?}");
  };
  let continued = db.invocation_prompt(continued.id).expect("read").expect("stored");
  assert_eq!(continued.event, InjectionEvent::Resume);
  (prompts(&db, run), base)
}

#[tokio::test(flavor = "multi_thread")]
async fn instructions_run_revision_at_resume_is_the_commit_the_agent_made_without_a_checkpoint() {
  let mut head = String::new();
  let (prompt, base) = resumed_prompt(|worktree| {
    // The agent committed its work itself: the worktree is clean, so no checkpoint records it.
    git(
      worktree,
      &["commit", "--allow-empty", "--quiet", "-m", "The agent's own commit"],
    );
    head = git(worktree, &["rev-parse", "HEAD"]);
  })
  .await;
  assert_ne!(head, base);
  assert!(prompt.contains(&format!("Compare {head}.")), "{prompt}");
}

#[tokio::test(flavor = "multi_thread")]
async fn instructions_run_revision_at_resume_is_the_base_while_nothing_moved_the_branch() {
  let (prompt, base) = resumed_prompt(|_| {}).await;
  assert!(prompt.contains(&format!("Compare {base}.")), "{prompt}");
}

#[tokio::test(flavor = "multi_thread")]
async fn instructions_say_what_the_run_delivers_as_its_task_says_it() {
  let project = project(&Setup::new("Find the cause of the crash."));
  let root = project.repo.path();
  ProjectDb::open(root)
    .expect("open")
    .update_task_organization(project.task, ChangeActor::Person, Timestamp::now(), |value| {
      value.with_delivers(Some(Deliverable::Report))
    })
    .expect("the task delivers a report");
  let (scheduler, mut events) = scheduler(root);
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Succeeded).await;
  let prompt = prompts(&ProjectDb::open(root).expect("open"), run);
  assert!(
    prompt.contains("This run delivers a report, not repository changes"),
    "{prompt}"
  );
  assert!(!prompt.contains("This run delivers repository changes"), "{prompt}");
}
