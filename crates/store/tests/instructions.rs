//! Instruction resources and what each agent invocation was told.

mod support;

use std::collections::BTreeSet;

use orchester_core::{
  id::{InvocationId, ResourceId},
  resource::{
    AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText, PublishError, Slug, StepKey, VersionNumber,
  },
  run::{InstructionVersion, InvocationPrompt, PromptPart, PromptSource},
  store::{ActivityStore, ResourceStore},
};
use orchester_store::StoreError;
use rusqlite::Connection;

use self::support::{at, model, name, started_run};

fn slug(value: &str) -> Slug {
  Slug::new(value).expect("valid slug")
}

fn step_rule(text: &str, pipeline: ResourceId) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("valid prompt"),
    AgentTargets::Profiles(BTreeSet::from([ResourceId::generate()])),
    BTreeSet::from([InjectionEvent::Resume]),
    InstructionScope::Step {
      pipeline,
      step: StepKey::new("implement").expect("valid key"),
    },
  )
  .expect("valid instruction")
}

#[test]
fn instructions_round_trip_as_drafts_and_versions() {
  let (_repo, mut db, _run, _execution) = started_run();
  let pipeline = ResourceId::generate();
  let blank = step_rule("", pipeline);
  let id = db
    .create_resource(slug("resume-checks"), name("Resume checks"), &blank, at(1))
    .expect("create")
    .id();
  assert_eq!(
    db.draft::<Instruction>(id).expect("read").expect("draft").body(),
    &blank
  );
  assert!(matches!(
    db.publish_draft::<Instruction>(id, at(2)),
    Err(StoreError::Publish(PublishError::BlankInstruction))
  ));
  let written = step_rule("Run `git status` on {{run.branch}} first.", pipeline);
  let draft = orchester_core::resource::Draft::new(written.clone(), None, at(3));
  db.save_draft(id, &draft).expect("save");
  let version = db.publish_draft::<Instruction>(id, at(4)).expect("publish");
  assert_eq!(version.number(), VersionNumber::FIRST);
  assert_eq!(version.body(), &written);
  assert_eq!(db.latest_version::<Instruction>(id).expect("read"), Some(version));
  assert_eq!(db.draft::<Instruction>(id).expect("read"), None);
}

#[test]
fn instructions_stored_before_profile_targets_read_unchanged() {
  let (repo, mut db, _run, _execution) = started_run();
  let rule = Instruction::new(
    PromptText::new("Keep commits small.").expect("valid prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("valid instruction");
  let id = db
    .create_resource(slug("git"), name("Git conventions"), &rule, at(1))
    .expect("create")
    .id();
  let raw = Connection::open(repo.path().join(".orchester/orchester.db")).expect("open raw database");
  let stored: String = raw
    .query_row("SELECT body FROM resource_drafts", [], |row| row.get(0))
    .expect("read");
  assert!(stored.contains(r#""schema":2"#), "{stored}");
  raw
    .execute(
      "UPDATE resource_drafts SET body = ?1",
      [stored.replace(r#""schema":2"#, r#""schema":1"#)],
    )
    .expect("write");
  assert_eq!(db.draft::<Instruction>(id).expect("read").expect("draft").body(), &rule);
}

#[test]
fn instructions_each_invocation_keeps_its_prompt_sources() {
  let (_repo, mut db, run, execution) = started_run();
  let invocation = db
    .start_invocation(run.id(), execution, &model(), at(10))
    .expect("start");
  assert_eq!(db.invocation_prompt(invocation.id).expect("read"), None);
  let prompt = InvocationPrompt {
    event: InjectionEvent::Start,
    sources: vec![
      PromptSource::Instruction {
        resource: ResourceId::generate(),
        part: PromptPart::ProjectRules,
        version: InstructionVersion::Published {
          number: VersionNumber::FIRST,
        },
      },
      PromptSource::Task { task: run.task() },
    ],
  };
  db.save_invocation_prompt(invocation.id, &prompt).expect("save");
  assert_eq!(db.invocation_prompt(invocation.id).expect("read"), Some(prompt.clone()));
  // Written once, before the CLI starts.
  assert!(db.save_invocation_prompt(invocation.id, &prompt).is_err());
  assert!(matches!(
    db.save_invocation_prompt(InvocationId::generate(), &prompt),
    Err(StoreError::InvocationNotFound(_))
  ));
}
