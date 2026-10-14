//! Answering in place as the panel draws it: a question in its raised box, *Open conversation* and *Send
//! answer* in one row and a line under it, and Enter in the own answer sending it (`Z4HeE3`); an approval and a
//! question answered while the panel stays open, each turned into its note with nothing of its waiting left
//! (`FETWe` in `HQ6Y1`); a lower item expanded scrolled into view under the pinned project header (`t7zhw`);
//! nothing answered in place while reading it again fails.

use lurq::core::ElementRect;
use orchester_app::routes::{ProjectPage, Route, TaskView};
use orchester_core::{
  id::RunId,
  question::GivenAnswer,
  run::{Answerer, DecidedBy, ToolApprovalDecision},
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::{Seeded, Window, click, expand_approval, ids_under, item_of, open, seeded, settle, text};

fn bounds(window: &mut Window, id: &str) -> ElementRect {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

fn shows(window: &mut Window, id: &str) -> bool {
  window.tree.get_element_by_id(id).is_some()
}

/// Expands the item of the task titled `title` around its question card: the item and the card's id.
pub(super) fn expand_question(window: &mut Window, title: &str) -> (String, String) {
  let item = item_of(window, title);
  click(window, &format!("{item}-action"));
  window.until("the card in place", |window| {
    !ids_under(window, &item, "cards-choice-").is_empty()
  });
  let mut cards = ids_under(window, &item, "cards-choice-");
  cards.sort_by_key(String::len);
  (item, cards[0].clone())
}

pub(super) fn press(window: &mut Window, key: &str, code: &str) {
  window
    .tree
    .key_down(key.to_owned(), code.to_owned(), false, false, false);
  window.pass();
}

#[test]
fn needs_you_panel_answers_a_question_in_its_box_with_its_answers_in_one_row() {
  let (mut window, _) = seeded("cards");
  open(&mut window);
  let (item, card) = expand_question(&mut window, "Pick where the docs button goes");
  let asked = format!("{card}-asked-0");
  window.until("the question's box", |window| shows(window, &asked));
  assert_eq!(text(&mut window, &asked), "Where should the docs button go?");
  let own = bounds(&mut window, &format!("{card}-own-0-box"));
  assert!((own.height - 32.0).abs() < 0.5, "the own answer is 32 high: {own:?}");
  click(&mut window, &format!("{card}-option-0-1"));
  let conversation = bounds(&mut window, &format!("{card}-conversation"));
  let send = bounds(&mut window, &format!("{card}-send"));
  let middle = |rect: &ElementRect| rect.y + rect.height / 2.0;
  assert!(
    (middle(&conversation) - middle(&send)).abs() < 0.5,
    "one row: {conversation:?}, {send:?}"
  );
  assert!(conversation.x + conversation.width < send.x, "Open conversation first");
  assert!(
    !shows(&mut window, &format!("{item}-conversation")),
    "the card's row carries Open conversation, not the item"
  );
  let hint = bounds(&mut window, &format!("{card}-context"));
  assert!(
    hint.y >= send.y + send.height - 0.5,
    "the line is under the row: {hint:?}"
  );
  assert_eq!(
    text(&mut window, &format!("{card}-context")),
    "Builder resumes at once with your answer · Enter sends"
  );
}

#[test]
fn needs_you_panel_sends_a_questions_own_answer_with_enter() {
  let (mut window, _) = seeded("cards");
  open(&mut window);
  let (_, card) = expand_question(&mut window, "Pick where the docs button goes");
  let own = format!("{card}-own-0");
  window.until("the own answer", |window| shows(window, &own));
  click(&mut window, &own);
  press(&mut window, "x", "KeyX");
  press(&mut window, "Enter", "Enter");
  // No runner here: the answer reaches none, and the card says why.
  window.until("the answer sent with Enter", |window| {
    shows(window, &format!("{card}-refused"))
  });
}

/// The run of the seeded task titled `title`.
fn run_of(db: &ProjectDb, title: &str) -> RunId {
  let task = db
    .tasks()
    .expect("the project's tasks")
    .into_iter()
    .find(|task| task.details().title.as_str() == title)
    .expect("the seeded task");
  db.runs_of_task(task.id())
    .expect("the task's runs")
    .into_iter()
    .find_map(Result::ok)
    .map(|run| run.id())
    .expect("its run")
}

/// The person approves once, through another way than the panel, the request of the task titled `title`.
fn approve(seeded: &Seeded, title: &str) {
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's database");
  let run = run_of(&db, title);
  let request = db
    .pending_tool_approvals(run)
    .expect("the run's requests")
    .into_iter()
    .next()
    .expect("a waiting request");
  assert!(
    db.claim_tool_approval(run, &request.id, DecidedBy::Person, Timestamp::now())
      .expect("the answer is recorded")
  );
  assert!(
    db.delivered_tool_approval(run, &request.id, ToolApprovalDecision::Approved)
      .expect("and delivered")
  );
}

/// The person answers the question of the task titled `title` with its first option, through another way.
fn answer(seeded: &Seeded, title: &str) {
  let mut db = ProjectDb::open(&seeded.repo).expect("the project's database");
  let run = run_of(&db, title);
  let asked = db
    .waiting_questions()
    .expect("the waiting questions")
    .into_iter()
    .find(|asked| asked.request.run() == run)
    .expect("its question");
  let first = asked.request.questions()[0].options()[0].label().to_owned();
  let answers = asked
    .request
    .answer(&[GivenAnswer {
      choices: vec![first],
      text: None,
    }])
    .expect("an answer it takes");
  let id = asked.request.id().to_owned();
  assert!(
    db.answer_questions((run, &id), &answers, (Answerer::Person, Timestamp::now()), None)
      .expect("the answer is recorded")
  );
}

fn key_of(seeded: &Seeded, title: &str) -> String {
  seeded
    .tasks
    .iter()
    .find(|task| task.title == title)
    .map(|task| task.key.clone())
    .expect("the seeded task")
}

/// Waits for `item`'s note to say `line`.
fn note_says(window: &mut Window, item: &str, line: &str) {
  window.until(&format!("the note to say {line}"), |window| {
    window
      .texts(&format!("{item}-line"))
      .is_some_and(|texts| texts.join(" ") == line)
  });
}

#[test]
fn needs_you_panel_turns_an_approval_answered_while_it_is_open_into_its_note() {
  let title = "Add serde to the webhook payloads";
  let (mut window, seeded) = seeded("cards");
  open(&mut window);
  let (item, card) = expand_approval(&mut window, title);
  window.until("the request's choices", |window| {
    shows(window, &format!("{card}-similar"))
  });
  approve(&seeded, title);
  window.projects().feed(seeded.project).saved();
  note_says(&mut window, &item, "Approved once · cargo add serde --features derive");
  let key = key_of(&seeded, title);
  assert_eq!(
    text(&mut window, &format!("{item}-detail")),
    format!("Builder runs it now; an approval can't be taken back. Recorded in {key} › Conversation.")
  );
  for gone in ["waiting", "collapse", "title", "task", "conversation"] {
    assert!(
      !shows(&mut window, &format!("{item}-{gone}")),
      "nothing of its waiting stays: {gone}"
    );
  }
  assert!(ids_under(&mut window, &item, "cards-approval-").is_empty());
  click(&mut window, &format!("{item}-open"));
  assert_eq!(
    window.navigation().route_now(),
    Route::Project(
      seeded.project,
      ProjectPage::Task(key.parse().expect("a task key"), TaskView::Conversation)
    )
  );
  window.until("the panel to close", |window| window.texts("needs-you-panel").is_none());
}

#[test]
fn needs_you_panel_turns_a_question_answered_while_it_is_open_into_its_note() {
  let title = "Pick where the docs button goes";
  let (mut window, seeded) = seeded("cards");
  open(&mut window);
  let (item, card) = expand_question(&mut window, title);
  window.until("the question's box", |window| shows(window, &format!("{card}-asked-0")));
  answer(&seeded, title);
  window.projects().feed(seeded.project).saved();
  note_says(&mut window, &item, "Answer sent to Builder · In the top bar");
  let detail = text(&mut window, &format!("{item}-detail"));
  let key = key_of(&seeded, title);
  assert!(
    detail.starts_with("Builder resumed at ")
      && detail.ends_with(&format!("Recorded in {key} › Conversation; follow up there.")),
    "{detail}"
  );
  assert!(!shows(&mut window, &format!("{item}-waiting")));
  assert!(ids_under(&mut window, &item, "cards-choice-").is_empty());
}

#[test]
fn needs_you_panel_keeps_a_lower_item_it_expands_in_view_under_the_pinned_header() {
  let (mut window, seeded) = seeded("cards");
  open(&mut window);
  let item = item_of(&mut window, "Read the docs of the sibling project");
  let action = format!("{item}-action");
  if let Some(mut element) = window.tree.get_element_by_id_mut(&action) {
    element.scroll_into_view();
  }
  window.pass();
  let before = bounds(&mut window, &item);
  click(&mut window, &action);
  window.until("the request's choices", |window| {
    ids_under(window, &item, "cards-approval-")
      .iter()
      .any(|id| id.ends_with("-similar"))
  });
  settle(&mut window);
  let (shown, list) = (bounds(&mut window, &item), bounds(&mut window, "needs-you-list"));
  assert!(
    shown.y < before.y - 0.5,
    "expanding brought it up: {shown:?}, was {before:?}"
  );
  assert!(
    shown.y >= list.y - 0.5,
    "its top is in the list's view: {shown:?} in {list:?}"
  );
  let whole = shown.y + shown.height <= list.y + list.height + 0.5;
  let from_top = (shown.y - list.y).abs() < 0.5;
  assert!(
    whole || from_top,
    "it shows whole, or from its top: {shown:?} in {list:?}"
  );
  let group = bounds(&mut window, &format!("needs-you-group-{}", seeded.project));
  assert!(
    group.y + group.height <= list.y + 0.5,
    "under the pinned header: {group:?}"
  );
}

/// Renames the project's table of tool requests to `to` from `from`, so reading the requests fails, or works again.
fn rename_requests(seeded: &Seeded, (from, to): (&str, &str)) {
  let database = seeded.repo.join(".orchester").join("orchester.db");
  rusqlite::Connection::open(database)
    .expect("the project's database")
    .execute(&format!("ALTER TABLE {from} RENAME TO {to}"), [])
    .expect("a real unreadable fixture");
}

#[test]
fn needs_you_panel_answers_nothing_in_place_once_reading_it_again_fails() {
  let (mut window, seeded) = seeded("cards");
  open(&mut window);
  let (item, card) = expand_approval(&mut window, "Add serde to the webhook payloads");
  let (approve, error) = (format!("{card}-approve"), format!("{item}-error"));
  window.until("Approve once", |window| shows(window, &approve));
  rename_requests(&seeded, ("tool_approvals", "tool_approvals_away"));
  window.projects().feed(seeded.project).saved();
  window.until("why it cannot be read", |window| shows(window, &error));
  assert!(
    !shows(&mut window, &approve),
    "what was read before no longer answers once reading it again failed"
  );
  assert!(shows(&mut window, &format!("{item}-conversation")));
  rename_requests(&seeded, ("tool_approvals_away", "tool_approvals"));
  window.projects().feed(seeded.project).saved();
  window.until("the card again", |window| shows(window, &approve));
  assert!(!shows(&mut window, &error));
}
