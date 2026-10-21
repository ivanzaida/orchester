//! Without a runner (its program does not exist), on the seed's `cards` scenario: an answer the runner cannot take
//! brings its item back, with why, on the card it was given on; and a project in another tab drops an item whose
//! answer settled as soon as the feed reads it again, which the settling itself starts: no run changed, nothing was
//! saved.

use orchester_core::{
  run::{DecidedBy, ToolApprovalDecision},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{
  answering::Answered,
  needs_you::{WaitingItem, WaitingKind},
};

use super::{Page, is_approval, places::expand_in_panel};

const APPROVAL: &str = "Add serde to the webhook payloads";

/// The waiting request of the task titled `title`.
fn request_of(page: &mut Page, title: &str) -> WaitingItem {
  let key = page.key_of(title);
  page.wait_for("the request", |item| {
    is_approval(item) && item.task.as_ref().is_some_and(|task| *task.key == key)
  })
}

#[test]
fn answered_refused_comes_back_with_its_reason_where_it_was_given() {
  let mut page = Page::with("cards", None);
  let item = request_of(&mut page, APPROVAL);
  let card = expand_in_panel(&mut page, APPROVAL, "cards-approval-");
  let approve = format!("{card}-approve");
  page.wait("Approve once", |window| {
    window.tree.get_element_by_id(&approve).is_some()
  });
  let before = page.count();
  // No runner takes it: the refusal may come back before the press's frame is laid out.
  page.click(&approve);
  let refused = format!("{card}-refused");
  page.wait("the item back with why", |window| {
    window.tree.get_element_by_id(&refused).is_some()
  });
  page.wait_until("the item back", |page| page.lists(&item));
  assert!(page.lists(&item), "the refused item waits again");
  assert_eq!(page.count(), before, "and counts again");
  assert_eq!(page.pill(), before, "the pill too");
  let services = page.window.launch.services().expect("the session started").clone();
  assert!(
    services.runners.answers_now(&page.repo).is_empty(),
    "a refusal leaves nothing marked"
  );
}

/// Records, as another way of answering would, that the person approved `item` once.
fn record_approval(page: &Page, item: &WaitingItem) {
  let WaitingKind::Approval(shown) = &item.kind else {
    panic!("an approval");
  };
  let mut db = ProjectDb::open(&page.repo).expect("the project's database");
  let (run, id) = (shown.approval.run, &shown.approval.id);
  assert!(
    db.claim_tool_approval(run, id, DecidedBy::Person, Timestamp::now())
      .expect("the answer is recorded")
  );
  assert!(
    db.delivered_tool_approval(run, id, ToolApprovalDecision::Approved)
      .expect("and delivered")
  );
}

#[test]
fn answered_for_another_tab_leaves_without_waiting_for_a_reading() {
  let mut page = Page::with("cards", None);
  let item = request_of(&mut page, APPROVAL);
  page.away();
  let services = page.window.launch.services().expect("the session started").clone();
  let WaitingKind::Approval(shown) = &item.kind else {
    panic!("an approval");
  };
  let answered = Answered::Approval {
    run: shown.approval.run,
    request: shown.approval.id.as_str().into(),
  };
  let mark = services.runners.answering(&page.repo, answered);
  page.wait_until("the marked item to leave", |page| !page.lists(&item));
  record_approval(&page, &item);
  mark.settle(true);
  // Only the settling tells the feed: no run changed and nothing was saved, so nothing else reads the tasks again.
  page.wait_until("the reading to take over", |page| {
    services.runners.answers_now(&page.repo).is_empty()
  });
  assert!(!page.lists(&item), "the reading no longer lists it");
}
