//! What the review of the Orchestrator's toolset asked to hold: a person's publication is published only as the
//! draft and the widenings they were shown, checked where the tool publishes; a merge proposal needs committed work;
//! a direct publication publishes the draft that was checked, by its id; and a duplicate is saved as a draft whose
//! first version follows the publication rule.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  id::ResourceId,
  orchestrator::{ActionOutcome, ChatAction, EntryKind, change::Publishing},
  resource::{Pipeline, ResourceKind},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_workspace::{
  app_mcp::{self, PersonDecision, ToolError, Translate},
  messages::Message,
  orchestrator::facts::{Facts, load_in},
};
use serde_json::{Value, json};

use super::{
  harness::{Orchestrator, Setup},
  proposal::outcomes,
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

/// When the draft of pipeline `pipeline` was saved.
fn saved(screen: &Orchestrator, pipeline: ResourceId) -> Timestamp {
  screen
    .db()
    .draft::<Pipeline>(pipeline)
    .expect("read")
    .expect("a draft")
    .updated_at()
}

/// What the draft of pipeline `pipeline`, saved at `saved`, widens, as a card reads it.
fn widens(screen: &Orchestrator, pipeline: ResourceId, saved: Timestamp) -> Vec<Message> {
  let action = ChatAction {
    change: Some(orchester_core::orchestrator::change::ProposedChange {
      subject: String::new(),
      change: orchester_core::orchestrator::change::Change::Publish {
        of: Publishing::Pipeline,
        resource: pipeline,
        version: 2,
        saved,
      },
    }),
    ..ChatAction::new("publish_pipeline", None, ActionOutcome::Waits)
  };
  let services = screen.window.launch.services().expect("launched services");
  match load_in(&screen.root, &services.programs, &action).expect("read") {
    Some(Facts::Publication(publication)) => publication.widens,
    other => panic!("a publication's facts: {other:?}"),
  }
}

/// `publish_pipeline` of `pipeline` as the person decided it: `decision`.
fn publish_as_person(
  screen: &Orchestrator,
  pipeline: ResourceId,
  decision: PersonDecision,
) -> Result<Value, ToolError> {
  let s = screen.window.launch.services().expect("launched services");
  let services = app_mcp::Services::new(
    s.workspace.clone(),
    s.runners.clone(),
    s.servers.clone(),
    s.programs.clone(),
    (s.changes.clone(), s.activity.clone(), s.jobs.clone()),
  )
  .decided_by_person(decision);
  let translate = Translate::new(|message| message.key.to_owned());
  let arguments = json!({ "project": screen.project.to_string(), "pipeline": pipeline.to_string() })
    .as_object()
    .cloned()
    .expect("an object");
  screen.window.data.runtime.block_on(async {
    app_mcp::setup::call(&services, &translate, "publish_pipeline", arguments)
      .await
      .expect("publish_pipeline is a setup tool")
  })
}

fn refused_as_changed(result: &Result<Value, ToolError>) -> bool {
  matches!(result, Err(ToolError::Refused { code, .. }) if code == "changed_since_proposed")
}

#[test]
fn orchestrator_person_publishes_only_the_draft_they_decided_on() {
  let screen = Orchestrator::new(Setup::Ready);
  let builder = screen.builder();
  let delivery = screen.pipeline(builder, ("delivery", "Delivery"));
  screen.pipeline_draft((delivery, builder), "Read README.md twice.");
  let decided = saved(&screen, delivery);
  let shown = widens(&screen, delivery, decided);
  assert!(!shown.is_empty(), "the draft widens: only a person publishes it");
  // The agent saves another draft after the person decided, before the tool publishes.
  std::thread::sleep(std::time::Duration::from_millis(5));
  screen.pipeline_draft((delivery, builder), "Delete the repository.");
  let published = publish_as_person(
    &screen,
    delivery,
    PersonDecision {
      saved: decided,
      version: 2,
      widens: shown,
    },
  );
  assert!(refused_as_changed(&published), "{published:?}");
  assert_eq!(
    screen.pipeline_version(delivery),
    1,
    "the later draft is not published as the person's"
  );
}

#[test]
fn orchestrator_person_publishes_only_what_they_were_shown_it_widens() {
  let screen = Orchestrator::new(Setup::Ready);
  let builder = screen.builder();
  let delivery = screen.pipeline(builder, ("delivery", "Delivery"));
  screen.pipeline_draft((delivery, builder), "Read README.md twice.");
  let decided = saved(&screen, delivery);
  let shown = widens(&screen, delivery, decided);
  let unshown = publish_as_person(
    &screen,
    delivery,
    PersonDecision {
      saved: decided,
      version: 2,
      widens: Vec::new(),
    },
  );
  assert!(refused_as_changed(&unshown), "{unshown:?}");
  assert_eq!(screen.pipeline_version(delivery), 1);
  publish_as_person(
    &screen,
    delivery,
    PersonDecision {
      saved: decided,
      version: 2,
      widens: shown,
    },
  )
  .expect("published as shown");
  assert_eq!(screen.pipeline_version(delivery), 2);
}

#[test]
fn orchestrator_merge_proposal_needs_committed_work() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let delivery = screen.delivery();
  let key = screen.draft_with(delivery, "Keep going", ("Keep working.", &["It keeps working."]));
  screen.auto_mode(true);
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
  screen.let_orchestrator_publish(true);
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
