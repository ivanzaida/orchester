//! What the review of the Orchestrator's toolset asked to hold: a merge proposal needs committed work; a duplicate is
//! published as its first version, as for any client (a pipeline holds no draft, D-032); and a published line keeps to
//! one line at a fractional scale.

use orchester_app::routes::OrchestratorChat;
use orchester_core::{
  orchestrator::{ActionOutcome, ChatAction, EntryKind},
  resource::{Pipeline, ResourceKind},
  store::ResourceStore,
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
fn orchestrator_duplicates_a_pipeline_as_its_first_version() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.delivery();
  screen.go(OrchestratorChat::New);
  // A copy that asks for nothing more than its source is published as v1 at once, as for any client (D-032).
  call(
    &mut screen,
    r#"Call duplicate_pipeline {"project":"Kontur","pipeline":"Delivery","name":"Hotfix"}"#,
    1,
  );
  let kept = actions(&screen);
  assert_eq!(
    (kept[0].tool.as_str(), &kept[0].outcome),
    ("duplicate_pipeline", &ActionOutcome::Done)
  );
  let db = screen.db();
  let hotfix = db
    .resources(ResourceKind::Pipeline)
    .expect("pipelines")
    .into_iter()
    .find(|found| found.name().as_str() == "Hotfix")
    .expect("the copy");
  assert_eq!(
    db.latest_version::<Pipeline>(hotfix.id())
      .expect("read")
      .map(|version| version.number().get()),
    Some(1),
    "the copy is its first version"
  );
  assert!(db.draft::<Pipeline>(hotfix.id()).expect("read").is_none(), "no draft");
}

#[test]
fn orchestrator_event_line_is_one_line_at_a_fractional_scale() {
  let mut screen = Orchestrator::new(Setup::Ready);
  // The verifier's window: 1440 wide at 1.5, where a title that fits was laid out on two lines.
  screen.window.tree.set_scale_factor(1.5);
  screen.window.tree.resize(2160, 1650);
  let delivery = screen.delivery();
  let instruction = screen.instruction(delivery, ("Review checklist", "## Tests\n- Run the suite once."));
  screen.instruction_draft((instruction, delivery), "## Tests\n- Run the suite twice.");
  screen.let_orchestrator_drive(true);
  screen.go(OrchestratorChat::New);
  call(
    &mut screen,
    r#"Call publish_instruction {"project":"Kontur","instruction":"Review checklist"}"#,
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
