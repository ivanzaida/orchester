//! What the Orchestrator chat sent or proposed shows in a task's conversation as its own (ORC-45), on the seeded
//! Kontur: a message it sent while *Let the Orchestrator answer agents* was on is the Orchestrator's, sent directly,
//! with the way back to its conversation; one it proposed waits in the task's conversation with *Apply* and *Dismiss*,
//! which decide the chat's own proposal, so applying it in either place applies it once and it then reads as proposed
//! by the Orchestrator and applied by the person; synthetic input cannot apply it; a dismissed one and an answer whose
//! question was answered meanwhile never read as sent; and an answer the chat gave names it on the question's card.
//! `get_conversation`, `get_run` and `list_questions` say the same.

use orchester_app::routes::{ProjectPage, Route, TaskTab, TaskView};
use orchester_core::{
  conversation::Message,
  id::{ChatId, RunId},
  orchestrator::{
    ActionOutcome, ChatAction, EntryKind, FromOrchestrator, SentWay,
    change::{Change, ProposedChange},
  },
  project::TaskKey,
  question::GivenAnswer,
  run::Answerer,
  store::ConversationStore,
  time::Timestamp,
};
use orchester_store::{KeptAction, ProjectDb};
use orchester_workspace::{
  app_mcp::{self, Translate},
  orchestrator::{
    proposal::{self, Decision},
    tools::ChatScope,
  },
  tasks::conversation::{
    composer::ComposerMode,
    orchestrator::{ProposalState, proposals},
  },
  workspace::OrchestratorConsent,
};
use serde_json::{Value, json};

use super::{
  Served,
  orchestrator_chat::{ASKING, actions, allow, chat, chat_call, task_run},
};

const TEXT: &str = "Keep the HubSpot owner.";
const PICKING: &str = "Pick where the docs button goes";

/// The seq of the last action of `scope`'s conversation.
fn last_action(db: &ProjectDb, scope: &ChatScope) -> i64 {
  db.chat_entries(scope.chat)
    .expect("entries")
    .into_iter()
    .filter(|entry| matches!(entry.kind, EntryKind::Action(_)))
    .map(|entry| entry.seq)
    .next_back()
    .expect("an action")
}

/// The person's `decision` on proposal `seq` of `scope`, taken as either place takes it (the chat's card, the task's
/// conversation), while the window runs.
fn decide(served: &mut Served, scope: &ChatScope, seq: i64, decision: Decision) -> Result<(), String> {
  let s = served.services();
  let services = app_mcp::Services::new(
    s.workspace.clone(),
    s.runners.clone(),
    s.servers.clone(),
    s.programs.clone(),
    (s.changes.clone(), s.activity.clone(), s.jobs.clone()),
  );
  let runners = s.runners.clone();
  let scope = scope.clone();
  let decided = served._runtime.spawn(async move {
    let translate = Translate::new(|message| message.key.to_owned());
    proposal::decide((&services, &translate, &runners), &scope, seq, decision).await
  });
  while !decided.is_finished() {
    served.pass();
    std::thread::sleep(std::time::Duration::from_millis(5));
  }
  let decided = served._runtime.block_on(decided).expect("the decision ends");
  decided.map_err(|message| message.key.to_owned())
}

/// The entries `get_conversation` gives of the task keyed `key`.
pub(super) fn entries(served: &mut Served, key: &str) -> Vec<Value> {
  let read = served.ok("get_conversation", &json!({ "project": "Kontur", "task": key }));
  read["entries"].as_array().expect("entries").clone()
}

/// The task keyed `key`'s Conversation, open in the window.
pub(super) fn open_conversation(served: &mut Served, project: orchester_core::id::ProjectId, key: &str) {
  let key: TaskKey = key.parse().expect("a task key");
  let navigation = served.link.navigation().expect("the window attached its navigation");
  navigation.go(Route::Project(
    project,
    ProjectPage::Task(key, TaskView::of(TaskTab::Conversation)),
  ));
  served.until("the conversation", |served| {
    served.tree.get_element_by_id("conversation-chat").is_some()
  });
}

/// The messages of the task whose latest message is `text`, if one is.
fn sent(db: &ProjectDb, task: orchester_core::id::TaskId, text: &str) -> Option<Message> {
  db.messages(task)
    .expect("messages")
    .into_iter()
    .find(|message| message.body().as_str() == text)
}

#[test]
fn sent_directly_names_the_orchestrator_and_how_it_went_out() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (task, run) = task_run(&db, ASKING);
  let scope = chat(&served, &mut db);
  allow(&served, OrchestratorConsent::AnswerAgents);
  // The message answers the agent's question: automation decides it only with the person's consent.
  served
    .services()
    .workspace
    .set_automation_answers_approvals(true)
    .expect("decision consent");
  let message = json!({ "project": "Kontur", "run": run.id().to_string(), "text": TEXT });
  chat_call(&mut served, &scope, "send_message", message).expect("sent at once");
  let stored = sent(&db, task.id(), TEXT).expect("the message");
  assert_eq!(
    stored.from_orchestrator(),
    Some(FromOrchestrator::new(scope.chat, SentWay::Direct))
  );
  let key = task.key().to_string();
  let entries = entries(&mut served, &key);
  let read = entries
    .iter()
    .find(|entry| entry["kind"] == "message" && entry["text"] == TEXT)
    .unwrap_or_else(|| panic!("the message: {entries:?}"));
  let author = &read["author"];
  assert_eq!(
    (&author["kind"], &author["sent"], &author["chat"]),
    (&json!("orchestrator"), &json!("direct"), &json!(scope.chat.to_string())),
    "{author}"
  );

  open_conversation(&mut served, seeded.project, &key);
  let id = format!("conversation-message-{}", stored.id());
  served.until("the Orchestrator's row", |served| {
    served.tree.get_element_by_id(&format!("{id}-row")).is_some()
  });
  assert!(
    served.tree.get_element_by_id(&format!("{id}-sent")).is_some()
      && served.tree.get_element_by_id(&format!("{id}-open-chat")).is_some(),
    "how it went out, and the way back to the chat"
  );
}

#[test]
fn sent_proposal_waits_in_the_task_conversation_until_the_person_applies_it() {
  let (mut served, seeded) = Served::seeded("tasks");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (task, run) = task_run(&db, ASKING);
  let scope = chat(&served, &mut db);
  let message = json!({ "project": "Kontur", "run": run.id().to_string(), "text": TEXT });
  chat_call(&mut served, &scope, "send_message", message).expect("proposed");
  let seq = last_action(&db, &scope);
  let key = task.key().to_string();
  let waiting = entries(&mut served, &key);
  let proposed = waiting
    .iter()
    .find(|entry| entry["kind"] == "orchestrator_proposal")
    .unwrap_or_else(|| panic!("the proposal: {waiting:?}"));
  assert_eq!(proposed["state"], "waits_for_person");
  assert_eq!(proposed["sent"], false);
  assert_eq!(proposed["proposed"]["message"]["text"], TEXT);
  assert!(sent(&db, task.id(), TEXT).is_none(), "nothing was sent");
  let read = served.ok("get_run", &json!({ "project": "Kontur", "run": run.id().to_string() }));
  assert_eq!(
    read["awaiting"]["orchestrator_proposals"].as_array().map(Vec::len),
    Some(1),
    "{read}"
  );

  open_conversation(&mut served, seeded.project, &key);
  let row = format!("conversation-proposal-{}-{seq}", scope.chat);
  served.until("the proposal's row", |served| {
    served.tree.get_element_by_id(&format!("{row}-apply")).is_some()
  });
  served.click_by_person(&format!("{row}-apply"));
  served.until("the applied message", |_| sent(&db, task.id(), TEXT).is_some());
  let stored = sent(&db, task.id(), TEXT).expect("the message");
  assert_eq!(
    stored.from_orchestrator(),
    Some(FromOrchestrator::new(scope.chat, SentWay::AppliedByPerson))
  );
  served.until("the chat's proposal applied", |_| {
    actions(&db, &scope)
      .last()
      .is_some_and(|action| action.outcome == ActionOutcome::Applied)
  });
  // One proposal behind both places: the chat's card cannot apply it again.
  let again = decide(&mut served, &scope, seq, Decision::Accept { widens: Vec::new() });
  assert_eq!(again, Err("orchestrator.proposal.taken".to_owned()));
  let read = entries(&mut served, &key);
  let applied = read
    .iter()
    .find(|entry| entry["kind"] == "message" && entry["text"] == TEXT)
    .expect("the applied message");
  assert_eq!(applied["author"]["sent"], "applied_by_person");
  assert!(
    !read.iter().any(|entry| entry["kind"] == "orchestrator_proposal"),
    "an applied proposal reads as its message only: {read:?}"
  );
}

#[test]
fn sent_proposal_refused_to_synthetic_input_and_dismissed_never_reads_as_sent() {
  let (folder, seeded) = Served::seed("tasks", None);
  let runner = folder.path().join("no-runner").join("orchester-runner");
  let mut served = Served::start_with(folder, &seeded.app_database, runner, true);
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (task, run) = task_run(&db, ASKING);
  let scope = chat(&served, &mut db);
  let message = json!({ "project": "Kontur", "run": run.id().to_string(), "text": TEXT });
  chat_call(&mut served, &scope, "send_message", message).expect("proposed");
  let seq = last_action(&db, &scope);
  let key = task.key().to_string();
  open_conversation(&mut served, seeded.project, &key);
  let row = format!("conversation-proposal-{}-{seq}", scope.chat);
  served.until("the proposal's row", |served| served.shows(&format!("{row}-apply")));
  served
    .click_by_client(&format!("{row}-apply"))
    .expect("the press reaches the window");
  served.until("the refusal", |served| served.shows(&format!("{row}-person-only")));
  assert!(sent(&db, task.id(), TEXT).is_none(), "synthetic input applies nothing");
  assert_eq!(actions(&db, &scope)[0].outcome, ActionOutcome::Waits);

  // Dismissed in the chat: the task's conversation keeps a muted line, nothing sent.
  decide(&mut served, &scope, seq, Decision::Dismiss).expect("dismissed");
  served.until("the muted line", |served| served.shows(&format!("{row}-settled")));
  let read = entries(&mut served, &key);
  let dismissed = read
    .iter()
    .find(|entry| entry["kind"] == "orchestrator_proposal")
    .expect("the dismissed proposal");
  assert_eq!(
    (&dismissed["state"], &dismissed["sent"]),
    (&json!("dismissed"), &json!(false))
  );
  assert!(sent(&db, task.id(), TEXT).is_none());
}

#[test]
fn sent_answer_proposal_goes_stale_once_its_question_is_answered() {
  let (mut served, seeded) = Served::seeded("cards");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (task, run) = task_run(&db, PICKING);
  let asked = db.questions(run.id()).expect("questions").remove(0);
  let scope = chat(&served, &mut db);
  let answer = json!({ "project": "Kontur", "run": run.id().to_string(), "choices": ["In the top bar"] });
  chat_call(&mut served, &scope, "answer_question", answer).expect("proposed");
  let listed = served.ok("list_questions", &json!({ "project": "Kontur" }));
  let question = listed["questions"]
    .as_array()
    .expect("questions")
    .iter()
    .find(|question| question["id"] == asked.request.id())
    .unwrap_or_else(|| panic!("the question: {listed}"))
    .clone();
  assert_eq!(question["proposed_answer"]["choices"], json!(["In the top bar"]));
  assert_eq!(question["proposed_answer"]["by"]["kind"], "orchestrator");
  let read = served.ok("get_run", &json!({ "project": "Kontur", "run": run.id().to_string() }));
  assert_eq!(
    read["awaiting"]["questions"][0]["proposed_answer"]["waits_for_person"], true,
    "{read}"
  );

  // The person answers it on its card meanwhile: the proposal no longer applies.
  let given = asked
    .request
    .answer(&[GivenAnswer {
      choices: vec!["In the top bar".to_owned()],
      text: None,
    }])
    .expect("answers");
  db.answer_questions(
    (run.id(), asked.request.id()),
    &given,
    (Answerer::Person, Timestamp::now()),
    None,
  )
  .expect("answered");
  let read = entries(&mut served, &task.key().to_string());
  let stale = read
    .iter()
    .find(|entry| entry["kind"] == "orchestrator_proposal")
    .unwrap_or_else(|| panic!("the proposal: {read:?}"));
  assert_eq!((&stale["state"], &stale["sent"]), (&json!("stale"), &json!(false)));
  let card = read
    .iter()
    .find(|entry| entry["kind"] == "question")
    .expect("the question's card");
  assert_eq!(card["answered_by"], "person", "the person's answer, not the chat's");
}

#[test]
fn sent_answer_of_the_orchestrator_names_it_on_the_card() {
  let (mut served, seeded) = Served::seeded("cards");
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's data");
  let (task, run) = task_run(&db, PICKING);
  let asked = db.questions(run.id()).expect("questions").remove(0);
  let scope = chat(&served, &mut db);
  let given = asked
    .request
    .answer(&[GivenAnswer {
      choices: vec!["In the top bar".to_owned()],
      text: None,
    }])
    .expect("answers");
  // As the runner records the answer of the chat's proposal the person applied, and the tool marks it.
  db.answer_questions(
    (run.id(), asked.request.id()),
    &given,
    (Answerer::Person, Timestamp::now()),
    None,
  )
  .expect("answered");
  assert!(
    db.mark_answered_by_orchestrator((run.id(), asked.request.id()), scope.chat)
      .expect("marked")
  );
  assert_eq!(
    db.questions(run.id()).expect("questions")[0].orchestrator,
    Some(FromOrchestrator::new(scope.chat, SentWay::AppliedByPerson)),
    "how it went out is who answered"
  );
  let key = task.key().to_string();
  let read = entries(&mut served, &key);
  let card = read
    .iter()
    .find(|entry| entry["kind"] == "question")
    .unwrap_or_else(|| panic!("the question's card: {read:?}"));
  assert_eq!(card["answered_by"], "orchestrator");
  assert_eq!(
    card["answered_by_orchestrator"],
    json!({ "kind": "orchestrator", "sent": "applied_by_person", "chat": scope.chat.to_string() })
  );

  open_conversation(&mut served, seeded.project, &key);
  let line = format!("cards-choice-{}-sent", asked.request.id());
  served.until("how the chat's answer went out", |served| served.shows(&line));
}

/// The chat's action on run `run` with `outcome`, a message proposed at `at` and checked as a follow-up; `claimed` and
/// `settled` as given.
fn kept(run: RunId, (seq, outcome): (i64, ActionOutcome), (claimed, settled): (bool, bool)) -> KeptAction {
  kept_as(run, (seq, outcome), (claimed, settled), Some("follow_up"))
}

/// [`kept`], checked as `sent_as` (`None`: kept before what it does was recorded).
fn kept_as(
  run: RunId,
  (seq, outcome): (i64, ActionOutcome),
  (claimed, settled): (bool, bool),
  sent_as: Option<&str>,
) -> KeptAction {
  let mut action = ChatAction::new("send_message", Some("KON-1".to_owned()), outcome);
  action.change = Some(Box::new(ProposedChange {
    subject: "Say hello".to_owned(),
    change: Change::Message {
      run,
      number: 1,
      text: format!("Message {seq}"),
      now: false,
      then: None,
      sent_as: sent_as.map(str::to_owned),
    },
  }));
  action.claimed = claimed.then(Timestamp::now);
  action.settled = settled.then(Timestamp::now);
  KeptAction {
    chat: ChatId::generate(),
    seq,
    at: Timestamp::from_unix_millis(seq),
    action,
  }
}

#[test]
fn sent_proposals_read_where_the_chats_action_stands_and_sent_ones_are_their_messages() {
  let run = RunId::generate();
  let refused = || ActionOutcome::Refused {
    code: "changed_since_proposed".to_owned(),
    reason: "The run moved on.".to_owned(),
  };
  let actions = [
    kept(run, (1, ActionOutcome::Waits), (false, false)),
    kept(run, (2, ActionOutcome::Waits), (true, false)),
    kept(run, (3, ActionOutcome::Dismissed), (false, true)),
    kept(run, (4, refused()), (false, true)),
    // Refused by the chat's own call: never proposed. Sent by the chat, or applied: shown as their messages.
    kept(run, (5, refused()), (false, false)),
    kept(run, (6, ActionOutcome::Done), (false, false)),
    kept(run, (7, ActionOutcome::Applied), (false, true)),
  ];
  let follow_up = ComposerMode::FollowUp;
  let states: Vec<(i64, ProposalState)> = proposals(&actions, (Some(&follow_up), &[]))
    .into_iter()
    .map(|proposal| (proposal.seq, proposal.state))
    .collect();
  assert_eq!(
    states,
    [
      (1, ProposalState::Waits),
      (2, ProposalState::Applying),
      (3, ProposalState::Dismissed),
      (4, ProposalState::Refused("The run moved on.".into())),
    ]
  );
  // Applying is refused, so the proposal is stale, once the run would take the message otherwise than it was checked
  // (here run again instead of following it up), once the run is not the task's latest, and for one kept before what
  // it does was recorded.
  let moved_on = ComposerMode::RunAgain;
  for (actions, offered) in [
    (
      vec![kept(run, (1, ActionOutcome::Waits), (false, false))],
      Some(&moved_on),
    ),
    (vec![kept(run, (1, ActionOutcome::Waits), (false, false))], None),
    (
      vec![kept_as(run, (1, ActionOutcome::Waits), (false, false), None)],
      Some(&follow_up),
    ),
  ] {
    let stale = proposals(&actions, (offered, &[]));
    assert_eq!(stale[0].state, ProposalState::Stale, "{offered:?}");
    assert!(!stale[0].waits(), "a stale proposal offers no Apply");
  }
  // A message checked to run the task again still applies once the run ended (the applied run-again card).
  let again = [kept_as(
    run,
    (1, ActionOutcome::Waits),
    (false, false),
    Some("run_again"),
  )];
  assert_eq!(proposals(&again, (Some(&moved_on), &[]))[0].state, ProposalState::Waits);
}
