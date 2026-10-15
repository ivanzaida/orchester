//! What a card of the Orchestrator chat reads of a publication it proposed (`b55NqO`): the draft's name and versions,
//! where the instruction applies, the lines its text removes and adds with a line around them; and that a draft saved
//! again no longer counts as the one proposed.

mod support;

use std::collections::BTreeSet;

use orchester_core::{
  id::ResourceId,
  name::Name,
  orchestrator::{
    ActionOutcome, ChatAction,
    change::{Change, ProposedChange, Publishing},
  },
  resource::{
    AgentTargets, Draft, InjectionEvent, Instruction, InstructionScope, PromptText, ResourceKind, Slug, StepKey,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::facts::{Facts, LineChange, Place, load_in};

fn instruction(pipeline: ResourceId, text: &str) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Step {
      pipeline,
      step: StepKey::new("implement").expect("key"),
    },
  )
  .expect("instruction")
}

/// *Review checklist* v1 on the fixture's step, and a draft of it saved at `saved`; its id.
fn drafted(db: &mut ProjectDb, saved: Timestamp) -> ResourceId {
  let pipeline = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  let created = db
    .create_resource(
      Slug::new("review-checklist").expect("slug"),
      Name::new("Review checklist").expect("name"),
      &instruction(pipeline, "## Tests\n- Ask for tests.\n- Run the suite.\n- Report."),
      support::at(10),
    )
    .expect("instruction")
    .id();
  let published = db
    .publish_draft::<Instruction>(created, support::at(11))
    .expect("publish");
  let body = instruction(
    pipeline,
    "## Tests\n- Name every test.\n- Ask for a test per module.\n- Run the suite.\n- Report.",
  );
  db.update_draft::<Instruction>(created, |_, _| Ok(Draft::new(body, Some(published.number()), saved)))
    .expect("draft");
  created
}

fn proposed(resource: ResourceId, saved: Timestamp) -> ChatAction {
  ChatAction {
    change: Some(ProposedChange {
      subject: "Review checklist".to_owned(),
      change: Change::Publish {
        of: Publishing::Instruction,
        resource,
        version: 2,
        saved,
      },
    }),
    ..ChatAction::new("publish_instruction", None, ActionOutcome::Waits)
  }
}

#[test]
fn orchestrator_facts_read_a_draft_where_it_applies_and_what_it_changes() {
  let mut fixture = support::Fixture::new();
  let saved = support::at(20);
  let resource = drafted(&mut fixture.db, saved);
  let facts = load_in(fixture.repo.path(), &Programs::processes(), &proposed(resource, saved))
    .expect("read")
    .expect("facts");
  let Facts::Publication(publication) = facts else {
    panic!("a publication's facts: {facts:?}");
  };
  assert_eq!(publication.name, "Review checklist");
  assert_eq!(publication.before_published, Some(support::at(11)));
  assert!(
    matches!(&publication.places[..], [Place::Step { step, .. }] if step == "Implement"),
    "{:?}",
    publication.places
  );
  let diff = publication.diff.expect("the text changes");
  assert_eq!((diff.removed, diff.added), (1, 2));
  let shown: Vec<(LineChange, &str)> = diff
    .lines
    .iter()
    .map(|line| (line.change, line.text.as_str()))
    .collect();
  assert_eq!(
    shown,
    vec![
      (LineChange::Same, "## Tests"),
      (LineChange::Removed, "- Ask for tests."),
      (LineChange::Added, "- Name every test."),
      (LineChange::Added, "- Ask for a test per module."),
      (LineChange::Same, "- Run the suite."),
    ],
    "the changed lines, removed first, with a line around them"
  );
  assert_eq!(diff.lines[1].old, Some(2));
  assert_eq!(diff.lines[3].new, Some(3));
}

#[test]
fn orchestrator_facts_drop_a_draft_saved_after_the_proposal() {
  let mut fixture = support::Fixture::new();
  let resource = drafted(&mut fixture.db, support::at(20));
  let facts = load_in(
    fixture.repo.path(),
    &Programs::processes(),
    &proposed(resource, support::at(19)),
  )
  .expect("read")
  .expect("facts");
  let Facts::Publication(publication) = facts else {
    panic!("a publication's facts: {facts:?}");
  };
  assert!(
    publication.diff.is_none(),
    "another draft than the proposed one shows no diff"
  );
  assert!(publication.widens.is_empty());
}
