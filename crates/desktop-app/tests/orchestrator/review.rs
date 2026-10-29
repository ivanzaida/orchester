//! What the review of the Orchestrator's toolset asked to hold: a person's publication (taken as the card takes it,
//! through `proposal::decide`) is published only as the draft and the widenings they were shown; a merge proposal needs
//! committed work; a direct publication publishes the draft that was checked, by its id; and a duplicate is saved as a
//! draft whose first version follows the publication rule.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  orchestrator::{ActionOutcome, ChatAction, EntryKind},
  resource::{Pipeline, ResourceKind},
  store::ResourceStore,
};
use orchester_workspace::{
  app_mcp::{self, Translate},
  messages::Message,
  orchestrator::{
    facts::{Facts, load_in},
    proposal::{self, Decision},
    tools::ChatScope,
  },
  workspace::OrchestratorConsent,
};

use super::{
  harness::{Orchestrator, Setup},
  proposal::{outcomes, proposal_card},
};

/// The conversation's actions, in order.
fn actions(screen: &Orchestrator) -> Vec<ChatAction> {
  let db = screen.db();
  let Some(listing) = db.chats().expect("chats").pop() else {
    return Vec::new();
  };
  db.chat_entries(listing.chat.id())
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Action(action) => Some(action),
      _ => None,
    })
    .collect()
}

/// Sends `call` and waits until its turn kept `count` actions and ended.
fn call(screen: &mut Orchestrator, call: &str, count: usize) {
  screen.send(call);
  screen.until("the call's action", |screen| outcomes(screen).len() == count);
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
}

/// The widenings the card of the conversation's latest action shows: what its draft widens, as the card reads it.
fn shown(screen: &Orchestrator) -> Vec<Message> {
  let action = actions(screen).pop().expect("an action");
  let services = screen.window.launch.services().expect("launched services");
  match load_in(&screen.root, &services.programs, &action).expect("read") {
    Some(Facts::Publication(publication)) => publication.widens,
    other => panic!("a publication's facts: {other:?}"),
  }
}

/// The person's decision on the conversation's latest action, as its card takes it, through `proposal::decide`.
pub(super) fn decide(screen: &Orchestrator, decision: Decision) -> Result<(), Message> {
  let db = screen.db();
  let chat = db.chats().expect("chats").remove(0).chat.id();
  let seq = db
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| matches!(entry.kind, EntryKind::Action(_)).then_some(entry.seq))
    .next_back()
    .expect("an action");
  let scope = ChatScope {
    project: screen.project,
    name: "Kontur".to_owned(),
    root: screen.root.clone(),
    chat,
    on_its_own: false,
  };
  let s = screen.window.launch.services().expect("launched services");
  let services = app_mcp::Services::new(
    s.workspace.clone(),
    s.runners.clone(),
    s.servers.clone(),
    s.programs.clone(),
    (s.changes.clone(), s.activity.clone(), s.jobs.clone()),
  );
  let translate = Translate::new(|message| message.key.to_owned());
  screen.window.data.runtime.block_on(proposal::decide(
    (&services, &translate, &s.runners),
    &scope,
    seq,
    decision,
  ))
}

fn refused_as_changed(outcome: &ActionOutcome) -> bool {
  matches!(outcome, ActionOutcome::Refused { code, .. } if code == "changed_since_proposed")
}

const PUBLISH_DELIVERY: &str = r#"Call publish_pipeline {"project":"Kontur","pipeline":"Delivery"}"#;

#[test]
fn orchestrator_person_publishes_only_the_draft_they_decided_on() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let builder = screen.builder();
  let delivery = screen.pipeline(builder, ("delivery", "Delivery"));
  screen.pipeline_draft((delivery, builder), "Read README.md twice.");
  screen.go(OrchestratorChat::New);
  call(&mut screen, PUBLISH_DELIVERY, 1);
  let widens = shown(&screen);
  assert!(!widens.is_empty(), "the draft widens: only a person publishes it");
  // The agent saves another draft after the person was shown this one, before they decide.
  std::thread::sleep(std::time::Duration::from_millis(5));
  screen.pipeline_draft((delivery, builder), "Delete the repository.");
  decide(&screen, Decision::Accept { widens }).expect("decided");
  assert!(
    refused_as_changed(&actions(&screen)[0].outcome),
    "{:?}",
    actions(&screen)[0]
  );
  assert_eq!(
    screen.pipeline_version(delivery),
    1,
    "the later draft is not published as the person's"
  );
}

#[test]
fn orchestrator_person_publishes_only_what_they_were_shown_it_widens() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let builder = screen.builder();
  let delivery = screen.pipeline(builder, ("delivery", "Delivery"));
  screen.pipeline_draft((delivery, builder), "Read README.md twice.");
  screen.go(OrchestratorChat::New);
  call(&mut screen, PUBLISH_DELIVERY, 1);
  decide(&screen, Decision::Accept { widens: Vec::new() }).expect("decided");
  assert!(
    refused_as_changed(&actions(&screen)[0].outcome),
    "{:?}",
    actions(&screen)[0]
  );
  assert_eq!(screen.pipeline_version(delivery), 1);
  call(&mut screen, PUBLISH_DELIVERY, 2);
  let widens = shown(&screen);
  decide(&screen, Decision::Accept { widens }).expect("decided");
  assert_eq!(actions(&screen)[1].outcome, ActionOutcome::Applied);
  assert_eq!(screen.pipeline_version(delivery), 2);
}

#[test]
fn orchestrator_merge_proposal_needs_committed_work() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let delivery = screen.delivery();
  let key = screen.draft_with(delivery, "Keep going", ("Keep working.", &["It keeps working."]));
  screen.allow_autopilot(true);
  screen.go(OrchestratorChat::New);
  call(
    &mut screen,
    &format!(r#"Call run_task {{"project":"Kontur","key":"{key}"}}"#),
    1,
  );
  let run = actions(&screen)[0].started.expect("auto mode started the run");
  call(
    &mut screen,
    &format!(r#"Call merge_run {{"project":"Kontur","run":"{run}"}}"#),
    2,
  );
  assert!(
    matches!(&actions(&screen)[1].outcome, ActionOutcome::Refused { code, .. } if code == "no_checkpoint"),
    "{:?}",
    actions(&screen)[1]
  );
}

#[test]
fn orchestrator_publishes_directly_the_draft_it_checked_by_its_id() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let builder = screen.builder();
  let delivery = screen.pipeline(builder, ("delivery", "Delivery"));
  // Another pipeline, listed first (by its slug), is named by Delivery's id: a call naming that id names Delivery.
  let impostor = screen.pipeline(builder, ("aa-impostor", &delivery.to_string()));
  screen.pipeline_draft((delivery, builder), "Read README.md twice.");
  screen.pipeline_draft((impostor, builder), "Read README.md three times.");
  screen.let_orchestrator(OrchestratorConsent::PublishDrafts, true);
  screen.let_automation_widen(true);
  screen.go(OrchestratorChat::New);
  call(
    &mut screen,
    &format!(r#"Call publish_pipeline {{"project":"Kontur","pipeline":"{delivery}"}}"#),
    1,
  );
  assert_eq!(actions(&screen)[0].outcome, ActionOutcome::Done);
  assert_eq!(screen.pipeline_version(delivery), 2, "the checked draft is published");
  assert_eq!(screen.pipeline_version(impostor), 1, "no other draft is");
}

#[test]
fn orchestrator_saves_a_duplicate_as_a_draft_and_proposes_its_first_version() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.delivery();
  screen.go(OrchestratorChat::New);
  call(
    &mut screen,
    r#"Call duplicate_pipeline {"project":"Kontur","pipeline":"Delivery","name":"Hotfix"}"#,
    2,
  );
  let kept = actions(&screen);
  assert_eq!(
    (kept[0].tool.as_str(), &kept[0].outcome),
    ("duplicate_pipeline", &ActionOutcome::Done)
  );
  assert_eq!(
    (kept[1].tool.as_str(), &kept[1].outcome),
    ("publish_pipeline", &ActionOutcome::Waits)
  );
  // The cards of one answer stand 10 apart, as every part of it (`b55NqO`).
  let first = proposal_card(&screen);
  let second = format!("{}1", first.trim_end_matches('0'));
  screen.until("both cards", |screen| screen.exists(&first) && screen.exists(&second));
  let (above, below) = (screen.bounds(&first), screen.bounds(&second));
  let gap = below.y - (above.y + above.height);
  assert!((gap - 10.0).abs() < 1.0, "the answer's cards are 10 apart: {gap}");
  let db = screen.db();
  let hotfix = db
    .resources(ResourceKind::Pipeline)
    .expect("pipelines")
    .into_iter()
    .find(|found| found.name().as_str() == "Hotfix")
    .expect("the copy");
  assert!(
    db.latest_version::<Pipeline>(hotfix.id()).expect("read").is_none(),
    "nothing is published yet"
  );
  assert!(
    db.draft::<Pipeline>(hotfix.id()).expect("read").is_some(),
    "the copy is a draft"
  );
}

#[test]
fn orchestrator_event_line_is_one_line_at_a_fractional_scale() {
  let mut screen = Orchestrator::new(Setup::Ready);
  // The verifier's window: 1440 wide at 1.5, where a title that fits was laid out on two lines.
  screen.window.tree.set_scale_factor(1.5);
  screen.window.tree.resize(2160, 1650);
  let builder = screen.builder();
  let delivery = screen.pipeline(builder, ("feature-delivery", "Feature delivery"));
  screen.pipeline_draft((delivery, builder), "Read README.md twice.");
  screen.let_orchestrator(OrchestratorConsent::PublishDrafts, true);
  screen.let_automation_widen(true);
  screen.go(OrchestratorChat::New);
  call(
    &mut screen,
    &format!(r#"Call publish_pipeline {{"project":"Kontur","pipeline":"{delivery}"}}"#),
    1,
  );
  let card = proposal_card(&screen);
  screen.until("the published line", |screen| screen.exists(&format!("{card}-time")));
  let (line, title) = (screen.bounds(&card), screen.bounds(&format!("{card}-title")));
  // Bounds are in logical pixels: one line of 12.5 text is well under half the line.
  assert!(
    (line.height - 32.0).abs() < 0.5 && title.height < line.height / 1.5,
    "the line is 32 high, its words on one line: {title:?} in {line:?}"
  );
}
