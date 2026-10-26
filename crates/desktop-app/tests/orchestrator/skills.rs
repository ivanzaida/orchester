//! Skills in the Orchestrator chat (ORC-79): the chat saves a skill's draft itself, marked as automation's, and
//! proposes its publication on a card that names the skill and its version, which the person publishes; with *Let the
//! Orchestrator publish drafts* on the chat publishes it itself.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  orchestrator::{
    ActionOutcome,
    change::{Change, Publishing},
  },
  resource::Skill,
  store::ResourceStore,
};
use orchester_workspace::{skills::data::ProjectSkills, workspace::OrchestratorConsent};

use super::{
  changes::{actions, call},
  harness::{Orchestrator, Setup},
  proposal::proposal_card,
};

const SAVE: &str = r#"Call save_skill_draft {"project":"Kontur","name":"Release notes","description":"Writing release notes.","body":"One line per change."}"#;
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
fn orchestrator_saves_a_skill_itself_and_the_person_publishes_it() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  call(&mut screen, SAVE, 1);
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
      version: 1,
      ..
    })
  ));
  assert_eq!(published(&screen), None);
  let card = proposal_card(&screen);
  screen.until("the card's facts", |screen| {
    screen.text(&format!("{card}-facts")).contains("Skill")
  });
  assert!(
    screen
      .text(&format!("{card}-title"))
      .contains("Publish Release notes v1?"),
    "the card names the skill and its version: {}",
    screen.text(&format!("{card}-title"))
  );
  screen.click(&format!("{card}-start"));
  screen.until("the publication", |screen| {
    actions(screen)[1].outcome == ActionOutcome::Applied
  });
  assert_eq!(published(&screen), Some(1), "the person's Publish published v1");
  assert!(!release_notes(&screen).1, "published, nothing is marked");
  // Decided, the card folds into a line that says the person did it.
  screen.until("the published line", |screen| {
    screen
      .text(&format!("{card}-title"))
      .contains("Published Release notes v1")
      && screen.exists(&format!("{card}-target"))
      && screen.text(&format!("{card}-target")) == "by you"
  });
  assert!(!screen.exists(&format!("{card}-start")), "nothing is left to decide");
}

#[test]
fn orchestrator_publishes_a_skill_itself_once_the_person_lets_it() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.let_orchestrator(OrchestratorConsent::PublishDrafts, true);
  screen.go(OrchestratorChat::New);
  call(&mut screen, SAVE, 1);
  call(&mut screen, PUBLISH, 2);
  assert_eq!(actions(&screen)[1].outcome, ActionOutcome::Done);
  assert_eq!(published(&screen), Some(1));
  let card = proposal_card(&screen);
  screen.until("the line", |screen| {
    screen.exists(&format!("{card}-title"))
      && screen
        .text(&format!("{card}-title"))
        .contains("Published Release notes v1")
  });
  assert!(!screen.exists(&format!("{card}-target")), "done by the Orchestrator, not by the person");
  assert!(!screen.exists(&format!("{card}-start")), "nothing is left to decide");
}
