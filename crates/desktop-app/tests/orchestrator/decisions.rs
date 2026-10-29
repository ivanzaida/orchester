//! A person's decision on a proposal of the Orchestrator ends and is kept whatever happens to its card: carried out
//! detached from the card, it settles even when the card goes away mid-way; and a claim an earlier session left
//! under way is settled at the start of the next by what happened.

use orchester_app::routes::{OrchestratorChat, ProjectPage, Route, TaskList};
use orchester_core::{
  orchestrator::{ActionOutcome, ChatAction, EntryKind},
  resource::Pipeline,
  run::RunStatus,
  store::{ResourceStore, RunStore},
  time::Timestamp,
};
use orchester_workspace::{app_mcp::Translate, orchestrator::recover};

use super::{
  harness::{Orchestrator, Setup},
  proposal::{outcomes, proposal_card},
};

/// The conversation's actions with their entries, in order.
fn actions(screen: &Orchestrator) -> Vec<(i64, ChatAction)> {
  let db = screen.db();
  let Some(listing) = db.chats().expect("chats").pop() else {
    return Vec::new();
  };
  db.chat_entries(listing.chat.id())
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Action(action) => Some((entry.seq, action)),
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
fn orchestrator_decision_ends_when_its_card_goes_away() {
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
  let run = actions(&screen)[0].1.started.expect("auto mode started the run");
  call(
    &mut screen,
    &format!(r#"Call cancel_run {{"project":"Kontur","run":"{run}"}}"#),
    2,
  );
  let card = proposal_card(&screen);
  screen.until("the card's facts", |screen| screen.exists(&format!("{card}-facts")));
  screen.click(&format!("{card}-start"));
  // The person leaves the conversation at once: its card goes away while the cancel is carried out.
  let project = screen.project;
  screen
    .window
    .navigation()
    .go(Route::Project(project, ProjectPage::Tasks(TaskList::default())));
  screen.until("the decision kept", |screen| {
    let (_, cancel) = &actions(screen)[1];
    cancel.outcome == ActionOutcome::Applied && cancel.claimed.is_none()
  });
  screen.until("the canceled run", |screen| {
    *screen.db().run(run).expect("read").expect("the run").status() == RunStatus::Cancelled
  });
}

#[test]
fn orchestrator_claim_an_earlier_session_left_is_settled_at_start() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let builder = screen.builder();
  let delivery = screen.pipeline(builder, ("delivery", "Delivery"));
  let hotfix = screen.pipeline(builder, ("hotfix", "Hotfix"));
  screen.pipeline_draft((delivery, builder), "Read README.md twice.");
  screen.pipeline_draft((hotfix, builder), "Read README.md twice.");
  screen.go(OrchestratorChat::New);
  let publish = |name: &str| format!(r#"Call publish_pipeline {{"project":"Kontur","pipeline":"{name}"}}"#);
  call(&mut screen, &publish("Delivery"), 1);
  call(&mut screen, &publish("Hotfix"), 2);
  call(&mut screen, &publish("Delivery"), 3);
  let services = screen.window.launch.services().expect("launched services");
  let start = services.session_start;
  let earlier = Timestamp::from_unix_millis(start.unix_millis() - 60_000);
  let chat = screen.db().chats().expect("chats").remove(0).chat.id();
  let seqs: Vec<i64> = actions(&screen).iter().map(|(seq, _)| *seq).collect();
  let mut db = screen.db();
  // Two claims an earlier session left: one whose publication happened before it ended, one that did not.
  db.claim_chat_action((chat, seqs[0]), earlier).expect("claim");
  db.claim_chat_action((chat, seqs[1]), earlier).expect("claim");
  // A claim of this session is under way: it is left alone.
  db.claim_chat_action((chat, seqs[2]), Timestamp::now()).expect("claim");
  db.publish_draft::<Pipeline>(delivery, Timestamp::now())
    .expect("published meanwhile");

  screen.window.data.runtime.block_on(recover::recover_all(
    services.workspace.clone(),
    (start, Translate::new(|message| message.key.to_owned())),
    services.runners.clone(),
  ));
  let kept = actions(&screen);
  assert_eq!(
    kept[0].1.outcome,
    ActionOutcome::Applied,
    "the version it would publish is published"
  );
  assert!(
    matches!(&kept[1].1.outcome, ActionOutcome::Refused { code, .. } if code == "interrupted"),
    "{:?}",
    kept[1].1
  );
  assert!(kept[0].1.claimed.is_none() && kept[1].1.claimed.is_none());
  assert!(kept[2].1.claimed.is_some(), "a claim of this session is not touched");
}
