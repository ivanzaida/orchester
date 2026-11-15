//! Skills in the Orchestrator chat (ORC-79): the chat saves a draft over a skill itself, marked as automation's, and
//! proposes its publication on a card that names the skill and its version, which the person publishes. A new skill is
//! created as its first version (ORC-816), which reaches agents as a publication does: the chat creates one only while
//! *Let the Orchestrator drive* is on, and with it on it publishes its drafts itself.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  orchestrator::{
    ActionOutcome,
    change::{Change, Publishing},
  },
  resource::{InstructionScope, Skill},
  store::ResourceStore,
};
use orchester_workspace::skills::data::ProjectSkills;

use super::{
  changes::{actions, call},
  harness::{Orchestrator, Setup},
  proposal::proposal_card,
};

const SAVE: &str = r#"Call save_skill_draft {"project":"Kontur","name":"Release notes","description":"Writing release notes.","body":"One line per change."}"#;
/// A draft over the skill *Release notes*, named by its slug.
const SAVE_OVER: &str = r#"Call save_skill_draft {"project":"Kontur","skill":"release-notes","description":"Writing release notes.","body":"One line per change, newest first."}"#;
/// Names the skill by its slug, as publish_skill takes it.
const PUBLISH: &str = r#"Call publish_skill {"project":"Kontur","skill":"release-notes"}"#;

/// The skill *Release notes*, and whether automation saved its draft.
fn release_notes(screen: &Orchestrator) -> (orchester_workspace::skills::data::SkillEntry, bool) {
  let skills = ProjectSkills::load_in(&screen.root).expect("skills");
  let entry = skills
    .entries
    .iter()
    .find(|entry| &*entry.name == "Release notes")
    .expect("the skill")
    .clone();
  let automated = skills.by_automation(&entry);
  (entry, automated)
}

/// The number of the skill's latest published version.
fn published(screen: &Orchestrator) -> Option<u32> {
  let (entry, _) = release_notes(screen);
  screen
    .db()
    .latest_version::<Skill>(entry.id)
    .expect("versions")
    .map(|version| version.number().get())
}

#[test]
fn orchestrator_creates_no_skill_while_it_does_not_drive() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  call(&mut screen, SAVE, 1);
  assert!(
    matches!(&actions(&screen)[0].outcome, ActionOutcome::Refused { code, .. } if code == "person_only"),
    "a new skill is its first version, which reaches agents: {:?}",
    actions(&screen)[0].outcome
  );
  assert!(
    ProjectSkills::load_in(&screen.root)
      .expect("skills")
      .entries
      .iter()
      .all(|entry| &*entry.name != "Release notes"),
    "nothing is saved"
  );
}

#[test]
fn orchestrator_saves_a_skill_itself_and_the_person_publishes_it() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let first = Skill::new(
    "Writing release notes.".into(),
    "One line per change.".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("a skill");
  orchester_workspace::skills::editor::save_in(&screen.root, None, "Release notes", &first).expect("v1");
  screen.go(OrchestratorChat::New);
  call(&mut screen, SAVE_OVER, 1);
  assert_eq!(
    actions(&screen)[0].outcome,
    ActionOutcome::Done,
    "a draft reaches no agent"
  );
  let (entry, automated) = release_notes(&screen);
  assert!(
    entry.state.has_draft() && automated,
    "the chat's draft, marked as automation's"
  );

  call(&mut screen, PUBLISH, 2);
  let proposed = actions(&screen).remove(1);
  assert_eq!(
    proposed.outcome,
    ActionOutcome::Waits,
    "nothing is published on the agent's word"
  );
  assert!(matches!(
    proposed.change.map(|change| change.change),
    Some(Change::Publish {
      of: Publishing::Skill,
      version: 2,
      ..
    })
  ));
  assert_eq!(published(&screen), Some(1), "the draft is not published");
  let card = proposal_card(&screen);
  screen.until("the card's facts", |screen| {
    screen.text(&format!("{card}-facts")).contains("Skill")
  });
  assert!(
    screen
      .text(&format!("{card}-title"))
      .contains("Publish Release notes v2?"),
    "the card names the skill and its version: {}",
    screen.text(&format!("{card}-title"))
  );
  screen.click(&format!("{card}-start"));
  screen.until("the publication", |screen| {
    actions(screen)[1].outcome == ActionOutcome::Applied
  });
  assert_eq!(published(&screen), Some(2), "the person's Publish published v2");
  assert!(!release_notes(&screen).1, "published, nothing is marked");
  // Decided, the card folds into a line that says the person did it.
  screen.until("the published line", |screen| {
    screen
      .text(&format!("{card}-title"))
      .contains("Published Release notes v2")
      && screen.exists(&format!("{card}-target"))
      && screen.text(&format!("{card}-target")) == "by you"
  });
  assert!(!screen.exists(&format!("{card}-start")), "nothing is left to decide");
}

#[test]
fn orchestrator_publishes_a_skill_itself_once_the_person_lets_it() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.let_orchestrator_drive(true);
  screen.go(OrchestratorChat::New);
  call(&mut screen, SAVE, 1);
  assert_eq!(
    actions(&screen)[0].outcome,
    ActionOutcome::Done,
    "it drives: the new skill is created"
  );
  assert_eq!(published(&screen), Some(1), "as its first version");
  call(&mut screen, SAVE_OVER, 2);
  call(&mut screen, PUBLISH, 3);
  assert_eq!(actions(&screen)[2].outcome, ActionOutcome::Done);
  assert_eq!(published(&screen), Some(2));
  let card = proposal_card(&screen);
  screen.until("the line", |screen| {
    screen.exists(&format!("{card}-title"))
      && screen
        .text(&format!("{card}-title"))
        .contains("Published Release notes v2")
  });
  assert!(
    !screen.exists(&format!("{card}-target")),
    "done by the Orchestrator, not by the person"
  );
  assert!(!screen.exists(&format!("{card}-start")), "nothing is left to decide");
}
