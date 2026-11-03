//! The side question pane (ORC-337, 12–12d): the run composer's *Side question* button opens it beside the
//! transcript with the task's questions about the run and their answers, saying the run's agent does not see them;
//! nothing of it is a message of the run's conversation until *Send to {agent}*, which posts one, and the button then
//! says it was sent. The `side-question` scenario keeps an answered question about KON-13's run, whose Builder waits.

use lurq::node::element::ElementRef;
use orchester_core::{
  conversation::Sender,
  id::TaskId,
  store::{ConversationStore, TaskStore},
};
use orchester_store::ProjectDb;

use super::{Seeded, Window, click, key_of, open, seeded, walk};

const TASK: &str = "Sync contacts with HubSpot";

/// The window on the `side-question` scenario, KON-13's Conversation shown and the pane opened from the composer.
fn opened() -> (Window, Seeded) {
  let (mut window, seeded) = seeded("side-question");
  let key = key_of(&seeded, TASK);
  open(&mut window, &seeded, &key, "composer-side-question");
  assert!(
    window.tree.get_element_by_id("side-question").is_none(),
    "closed until asked for"
  );
  click(&mut window, "composer-side-question");
  window.until("the pane's answer", |window| {
    !pane_ids(window, "side-answer-send-").is_empty()
  });
  (window, seeded)
}

/// Every element id in the pane that starts with `prefix`.
fn pane_ids(window: &mut Window, prefix: &str) -> Vec<String> {
  let Some(pane) = window.tree.get_element_by_id("side-question") else {
    return Vec::new();
  };
  let mut found = Vec::new();
  walk(pane, &mut |element: ElementRef<'_>| {
    if let Some(id) = element.id().filter(|id| id.starts_with(prefix)) {
      found.push(id.to_owned());
    }
  });
  found
}

fn pane_text(window: &mut Window) -> String {
  window.texts("side-question").unwrap_or_default().join("\n")
}

/// The person's messages to `TASK`, as the project keeps them.
fn persons_messages(seeded: &Seeded) -> Vec<String> {
  let db = ProjectDb::open(&seeded.repo).expect("the project database");
  let task = task_id(&db, seeded);
  db.messages(task)
    .expect("the messages")
    .into_iter()
    .filter(|message| matches!(message.sender(), Sender::Person { .. }))
    .map(|message| message.body().as_str().to_owned())
    .collect()
}

fn task_id(db: &ProjectDb, seeded: &Seeded) -> TaskId {
  let key = key_of(seeded, TASK);
  db.tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| *task.key() == key)
    .expect("the task")
    .id()
}

#[test]
fn side_question_pane_opens_beside_the_transcript_with_the_answer() {
  let (mut window, seeded) = opened();
  let text = pane_text(&mut window);
  assert!(text.contains("Side question"), "{text}");
  assert!(text.contains("KON-13 · run"), "{text}");
  assert!(
    text.contains("Builder doesn't see this, and its turn goes on."),
    "the scope line says where the question goes: {text}"
  );
  assert!(
    text.contains("Why did Builder change src/fixtures/retry.json?"),
    "{text}"
  );
  assert!(text.contains("only you see it"), "{text}");
  assert!(text.contains("claude-haiku-5-5"), "the answer's model: {text}");
  assert!(
    text.contains("The retry change itself is only in src/sender.rs"),
    "{text}"
  );
  for source in ["src/fixtures/retry.json", "cargo test -p webhooks retry", "Builder"] {
    assert!(text.contains(source), "{source} as a source: {text}");
  }
  assert!(
    text.contains("Send to Builder") && text.contains("Copy") && text.contains("Not sent"),
    "{text}"
  );
  assert!(text.contains("Reads only · changes nothing"), "{text}");
  assert_eq!(
    pane_ids(&mut window, "side-source-open-").len(),
    1,
    "only the changed file opens"
  );
  // The transcript and its composer stay beside the pane.
  assert!(window.tree.get_element_by_id("conversation").is_some());
  assert!(window.tree.get_element_by_id("composer-box").is_some());
  assert!(persons_messages(&seeded).is_empty(), "nothing reached Builder");
}

#[test]
fn side_question_send_passes_it_on_as_one_message() {
  let (mut window, seeded) = opened();
  let send = pane_ids(&mut window, "side-answer-send-").remove(0);
  click(&mut window, &send);
  window.until("Sent to Builder", |window| {
    !pane_ids(window, "side-answer-sent-").is_empty()
  });
  let text = pane_text(&mut window);
  assert!(text.contains("Sent to Builder ·"), "{text}");
  assert!(!text.contains("Not sent"), "{text}");
  let messages = persons_messages(&seeded);
  assert_eq!(messages.len(), 1, "one ordinary message: {messages:?}");
  assert!(messages[0].starts_with("From a side question"), "{}", messages[0]);
  assert!(messages[0].contains("Why did Builder change src/fixtures/retry.json?"));
  assert!(messages[0].contains("Side answer: Builder's third test run failed"));
}

#[test]
fn side_question_closes_and_keeps_its_questions() {
  let (mut window, _) = opened();
  click(&mut window, "side-question-close");
  window.until("the pane closed", |window| {
    window.tree.get_element_by_id("side-question").is_none()
  });
  click(&mut window, "composer-side-question");
  window.until("the pane again", |window| {
    !pane_ids(window, "side-answer-send-").is_empty()
  });
  assert!(pane_text(&mut window).contains("Why did Builder change"));
}
