//! Approving the Orchestrator's proposal in the chat (ORC-96), on the seeded Kontur: a merge the chat proposed is
//! merged once the person's next message approves it and the chat applies it with `apply_proposal`, as the card's
//! *Apply* merges it; the card says it was applied by the person in the chat and quotes their message. A turn
//! Orchester started by itself and a message automation sent through the window approve nothing.

use lurq::node::element::ElementRef;
use orchester_core::{
  conversation::MessageBody,
  id::ChatMessageId,
  orchestrator::{ActionOutcome, EntryKind},
  run::Answerer,
  store::MergeStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{app_mcp::ToolError, orchestrator::tools::ChatScope};
use serde_json::json;

use super::{
  Served,
  orchestrator_chat::{actions, chat, chat_call, task_run},
};

/// The seeded task whose latest run succeeded and waits to be merged.
const READY: &str = "Show the signed-in account";

/// The person's message `text`, sent `by` them or automation, kept in `scope`'s conversation with its turn thinking.
fn says(db: &mut ProjectDb, scope: &ChatScope, (text, by): (&str, Answerer)) -> ChatMessageId {
  let conversation = db.chat(scope.chat).expect("read").expect("the conversation");
  let message = ChatMessageId::generate();
  let body = MessageBody::new(text).expect("text");
  db.keep_chat_message_with(&conversation, (message, &body), (&[], by), Timestamp::now())
    .expect("kept");
  message
}

/// The entry of `scope`'s newest action, and the person's message whose turn kept it.
fn newest_action(db: &ProjectDb, scope: &ChatScope) -> (i64, ChatMessageId) {
  let entries = db.chat_entries(scope.chat).expect("entries");
  let seq = entries
    .iter()
    .rev()
    .find(|entry| matches!(entry.kind, EntryKind::Action(_)))
    .expect("an action")
    .seq;
  let turn = entries
    .iter()
    .rev()
    .filter(|entry| entry.seq < seq)
    .find_map(|entry| match entry.kind {
      EntryKind::Person { message, .. } => Some(message),
      _ => None,
    })
    .expect("the turn it was kept in");
  (seq, turn)
}

fn code(error: &ToolError) -> &str {
  match error {
    ToolError::Refused { code, .. } => code,
    other => panic!("a refusal: {other:?}"),
  }
}

/// Whether `element` or one of its descendants holds text that contains `text`.
fn holds(element: ElementRef<'_>, text: &str) -> bool {
  element.text_content().is_some_and(|content| content.contains(text))
    || element.children().into_iter().any(|child| holds(child, text))
}

#[test]
fn chat_approval_merges_the_run_the_person_approved_and_the_card_says_so() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (_, run) = task_run(&db, READY);
  let scope = chat(&served, &mut db);
  let merge = json!({ "project": "Kontur", "run": run.id().to_string() });
  chat_call(&mut served, &scope, "merge_run", merge).expect("proposed");
  let (seq, turn) = newest_action(&db, &scope);
  let approved = says(&mut db, &scope, ("merge it", Answerer::Person));
  let told = chat_call(
    &mut served,
    &scope,
    "apply_proposal",
    json!({ "project": "Kontur", "proposal": seq }),
  )
  .expect("applied");
  assert_eq!(told["applied"], true, "{told}");
  assert!(db.run_merge(run.id()).expect("read").is_some(), "the run is merged");
  let applied = actions(&db, &scope).pop().expect("the proposal");
  assert_eq!(
    applied.outcome,
    ActionOutcome::Applied,
    "the person's decision, as the card's Apply"
  );
  assert_eq!(applied.approved_in.map(|approval| approval.message), Some(approved));

  let navigation = served.link.navigation().expect("the window attached its navigation");
  navigation.go(orchester_app::routes::Route::Project(
    seeded.project,
    orchester_app::routes::ProjectPage::Orchestrator(orchester_app::routes::OrchestratorChat::Chat(scope.chat)),
  ));
  // Applied, the card folds into its line, which says how it was applied and, opened, quotes the approval.
  let card = format!("orchestrator-action-{turn}-0");
  served.until("the card's line", |served| {
    served
      .tree
      .get_element_by_id(&format!("{card}-target"))
      .is_some_and(|target| holds(target, "by you in the chat"))
  });
  served
    .tree
    .get_element_by_id_mut(&format!("{card}-line"))
    .expect("the line")
    .click();
  served.until("the quoted approval", |served| {
    served
      .tree
      .get_element_by_id(&format!("{card}-reason"))
      .is_some_and(|note| holds(note, "“merge it”"))
  });
}

#[test]
fn chat_approval_is_refused_in_a_turn_orchester_started_and_for_automations_message() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (_, run) = task_run(&db, READY);
  let scope = chat(&served, &mut db);
  let merge = json!({ "project": "Kontur", "run": run.id().to_string() });
  chat_call(&mut served, &scope, "merge_run", merge).expect("proposed");
  let (seq, _) = newest_action(&db, &scope);
  let apply = json!({ "project": "Kontur", "proposal": seq });
  let on_its_own = ChatScope {
    on_its_own: true,
    ..scope.clone()
  };
  let refused = chat_call(&mut served, &on_its_own, "apply_proposal", apply.clone()).expect_err("nobody asked");
  assert_eq!(code(&refused), "not_persons_turn");
  says(&mut db, &scope, ("merge it", Answerer::Automation));
  let refused = chat_call(&mut served, &scope, "apply_proposal", apply).expect_err("automation's words");
  assert_eq!(code(&refused), "automation_input");
  assert!(db.run_merge(run.id()).expect("read").is_none(), "nothing merged");
  assert!(actions(&db, &scope).pop().expect("the proposal").waits());
}
