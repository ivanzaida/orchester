//! Tool requests as the run's approval mode answered them, on the `cards-approval-modes` scenario
//! (`docs/design/approval-modes.md`): a request that came to the person names why and is approved with *Approve*; one
//! the mode approved is a quiet row of its kind; Autopilot's rows of a step show three and how many more follow.

use lurq::app::events::ScrollPhase;

use super::{Window, cards, click, laid_out, opened_in, text};

const SCENARIO: &str = "cards-approval-modes";

/// Scrolls the chat up until `id` is inside it: the chat opened on the card that waits, under the rows.
fn into_view(window: &mut Window, id: &str) {
  let chat = laid_out(window, "conversation-chat");
  let (x, y) = (chat.x + chat.width / 2.0, chat.y + chat.height / 2.0);
  for _ in 0..40 {
    let shown = laid_out(window, id);
    if shown.y >= chat.y && shown.y + shown.height <= chat.y + chat.height {
      return;
    }
    window.tree.mouse_move(x, y);
    window.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::Start);
    window.tree.scroll(x, y, 0.0, 60.0, ScrollPhase::Scroll);
    window.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::End);
    window.pass();
    window.pass();
  }
  panic!("{id} never came into the chat's view");
}

/// The card whose reason line shows, once it does.
fn came_card(window: &mut Window) -> String {
  window.until("the card's reason", |window| {
    cards(window, "cards-approval-").iter().any(|id| id.ends_with("-came"))
  });
  cards(window, "cards-approval-")
    .into_iter()
    .find_map(|id| id.strip_suffix("-came").map(str::to_owned))
    .expect("a card that came to the person")
}

/// The ids of the quiet rows shown, by their lead.
fn leads(window: &mut Window) -> Vec<String> {
  cards(window, "cards-approval-")
    .into_iter()
    .filter(|id| id.ends_with("-lead"))
    .collect()
}

#[test]
fn card_approval_came_to_you_names_why_and_approves_with_approve() {
  for (title, reason) in [
    (
      "Add retry to webhook sender",
      "Came to you: C:\\Users\\dana\\.cargo\\config.toml is outside the worktree, its Sources and Builder's temp \
       folders. Autopilot never approves these.",
    ),
    (
      "Cap Retry-After at 60 s",
      "The reviewer wasn't sure: “Sends a request to an outside host; can't tell whether it only tests the retry \
       path.”",
    ),
    (
      "Add a JSON client",
      "The reviewer denied it: “Adds a new dependency the task doesn't ask for.” Not on Builder's allowed list.",
    ),
    ("Read the deploy key", "Not on Builder's allowed list."),
    (
      "Publish the retry branch",
      "Pushes to a remote always come to you, in Autopilot too.",
    ),
  ] {
    let (mut window, ..) = opened_in(SCENARIO, title, "cards-approval-");
    let card = came_card(&mut window);
    assert_eq!(text(&mut window, &format!("{card}-came-text")), reason, "{title}");
    assert_eq!(text(&mut window, &format!("{card}-approve")), "Approve", "{title}");
  }
  let (mut window, ..) = opened_in(SCENARIO, "Move main to the fix", "cards-approval-");
  let card = came_card(&mut window);
  let said = text(&mut window, &format!("{card}-came-text"));
  assert!(
    said.starts_with("Only the run's branch orchester/")
      && said.ends_with(" is the run's own; other refs come to you."),
    "{said}"
  );
}

#[test]
fn card_approval_rows_say_how_the_mode_approved_each_request() {
  let (mut window, ..) = opened_in(SCENARIO, "Cap Retry-After at 60 s", "cards-approval-");
  window.until("the two quiet rows", |window| leads(window).len() == 2);
  let mut said: Vec<String> = leads(&mut window).iter().map(|lead| text(&mut window, lead)).collect();
  said.sort();
  assert_eq!(said, ["Allowed by the profile", "Approved by the reviewer"]);
  let reviewer = leads(&mut window)
    .into_iter()
    .find(|lead| text(&mut window, lead) == "Approved by the reviewer")
    .and_then(|lead| lead.strip_suffix("-lead").map(str::to_owned))
    .expect("the reviewer's row");
  assert_eq!(
    text(&mut window, &format!("{reviewer}-asked")),
    "rm -rf target/fixtures-tmp"
  );
  let detail = text(&mut window, &format!("{reviewer}-detail"));
  assert!(
    detail.starts_with("· “Deletes generated fixtures inside the worktree.” · "),
    "{detail}"
  );
}

#[test]
fn card_approval_autopilot_rows_of_a_step_show_three_and_how_many_more() {
  let (mut window, ..) = opened_in(SCENARIO, "Add retry to webhook sender", "cards-approval-");
  window.until("the step's Autopilot rows", |window| leads(window).len() == 3);
  for lead in leads(&mut window) {
    assert_eq!(text(&mut window, &lead), "Approved by autopilot");
  }
  let more = cards(&mut window, "cards-approval-")
    .into_iter()
    .find(|id| id.ends_with("-more"))
    .expect("how many more follow");
  assert_eq!(text(&mut window, &more), "· 3 more");
  into_view(&mut window, &more);
  click(&mut window, &more);
  window.until("all six rows", |window| leads(window).len() == 6);
  assert!(
    !cards(&mut window, "cards-approval-")
      .iter()
      .any(|id| id.ends_with("-more")),
    "nothing more to show"
  );
}
