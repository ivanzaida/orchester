//! Project instructions in the prompts agents receive: those injected at start in a new session's
//! prompt on standard input, those injected at resume when a session continues, each invocation
//! with the sources of its prompt, and the versions the run pinned when it started. Against
//! `orchester-fake-codex`; needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test instructions`.

mod support;

use std::{collections::BTreeSet, num::NonZeroU32, path::Path};

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{AgentTargets, Draft, InjectionEvent, Instruction, InstructionScope, PromptText, Slug},
  run::{PromptPart, PromptSource, RunStatus},
  store::{ActivityStore, ResourceStore},
  time::Timestamp,
};
use orchester_ipc::AttachSession;
use orchester_store::ProjectDb;

use self::support::{Setup, killed_mid_run, project, reported, scheduler, wait_for};

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
