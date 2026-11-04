//! Tool requests as the run's approval mode answered them, on the `cards-approval-modes` scenario
//! (`docs/design/approval-modes.md`): a request that came to the person is the compact card that says what the agent
//! asks and why it came, approved with *Approve*, its other answers under *More answers*; one the mode approved is a
//! quiet row of its kind; Autopilot's rows of a step show three and how many more follow.

use lurq::app::events::ScrollPhase;
use orchester_app::ui::icon::Icon;

use super::{Window, cards, click, glyphs, laid_out, opened_in, text};

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

const NOT_ALLOWED: &str = "Builder asks to run a command not on its allowed list";

#[test]
fn card_approval_came_to_you_names_why_and_approves_with_approve() {
  for (task, (title, glyph), reason) in [
    (
      "Add retry to webhook sender",
      ("Builder asks to edit a file outside the worktree", Icon::FolderLock),
      "Came to you: C:\\Users\\dana\\.cargo\\config.toml is outside the worktree, its Sources and Builder's temp \
       folders. Autopilot never approves these.",
    ),
    (
      "Cap Retry-After at 60 s",
      (NOT_ALLOWED, Icon::ShieldQuestion),
      "The reviewer wasn't sure: “Sends a request to an outside host; can't tell whether it only tests the retry \
       path.”",
    ),
    (
      "Add a JSON client",
      (NOT_ALLOWED, Icon::ShieldX),
      "The reviewer denied it: “Adds a new dependency the task doesn't ask for.” Not on Builder's allowed list.",
    ),
    (
      "Read the deploy key",
      (NOT_ALLOWED, Icon::ShieldX),
      "Not on Builder's allowed list.",
    ),
    (
      "Publish the retry branch",
      ("Builder asks to push to a remote", Icon::Upload),
      "Pushes to a remote always come to you, in Autopilot too.",
    ),
  ] {
    let (mut window, ..) = opened_in(SCENARIO, task, "cards-approval-");
    let card = came_card(&mut window);
    assert_eq!(text(&mut window, &format!("{card}-title")), title, "{task}");
    assert!(
      glyphs(&mut window, &format!("{card}-head")).contains(&glyph.glyph().to_string()),
      "{task}: the head's glyph is {glyph:?}"
    );
    let meta = text(&mut window, &format!("{card}-meta"));
    assert!(meta.contains("Implement · "), "{task}: {meta}");
    assert_eq!(text(&mut window, &format!("{card}-came-text")), reason, "{task}");
    assert_eq!(text(&mut window, &format!("{card}-approve")), "Approve", "{task}");
    assert_eq!(text(&mut window, &format!("{card}-deny")), "Deny", "{task}");
  }
  let (mut window, ..) = opened_in(SCENARIO, "Move main to the fix", "cards-approval-");
  let card = came_card(&mut window);
  assert_eq!(
    text(&mut window, &format!("{card}-title")),
    "Builder asks to change another branch"
  );
  assert_eq!(
    text(&mut window, &format!("{card}-action")),
    "git update-ref refs/heads/main 4e1c9d2"
  );
  let said = text(&mut window, &format!("{card}-came-text"));
  assert!(
    said.starts_with("Only the run's branch orchester/")
      && said.ends_with(" is the run's own; other refs come to you."),
    "{said}"
  );
}

/// A request the reviewer could not judge says so, not that it was unsure, with why — the reviewer's account is at its
/// weekly plan limit, with when it resets — and *Reviewer settings*, which opens Project settings › Runs, whose
/// Reviewer row says the same (ORC-611).
#[test]
fn card_approval_the_reviewer_that_could_not_run_says_why_and_opens_its_settings() {
  let (mut window, ..) = opened_in(SCENARIO, "Retry uploads on 503", "cards-approval-");
  let card = came_card(&mut window);
  assert!(
    glyphs(&mut window, &format!("{card}-head")).contains(&Icon::ShieldAlert.glyph().to_string()),
    "the head's glyph is a shield with an alert"
  );
  let came = format!("{card}-came-text");
  window.until("why the reviewer could not run", |window| {
    text(window, &came) != "The reviewer couldn't run."
  });
  let said = text(&mut window, &came);
  assert!(
    said.starts_with("The reviewer couldn't run: its account reached its plan limit, which resets ")
      && said.ends_with('.'),
    "{said}"
  );
  let link = format!("{card}-reviewer-settings");
  assert_eq!(text(&mut window, &link), "Reviewer settings");
  click(&mut window, &link);
  window.until("the Reviewer row says why", |window| {
    window.texts("settings-auto-reviewer-hint").is_some_and(|hint| {
      hint
        .join(" ")
        .starts_with("The reviewer can't run: its account reached its plan limit, which resets ")
    })
  });
}

/// A request whose review timed out or whose answer could not be read says so in plain words, not that the reviewer
/// could not run, and keeps *Reviewer settings* (ORC-586).
#[test]
fn card_approval_a_failed_review_names_how_it_failed() {
  for (task, said) in [
    ("Retry downloads on 503", "The reviewer didn't answer in time."),
    ("Bump the JSON parser", "The reviewer's answer couldn't be read."),
  ] {
    let (mut window, ..) = opened_in(SCENARIO, task, "cards-approval-");
    let card = came_card(&mut window);
    assert_eq!(text(&mut window, &format!("{card}-title")), NOT_ALLOWED, "{task}");
    assert!(
      glyphs(&mut window, &format!("{card}-head")).contains(&Icon::ShieldAlert.glyph().to_string()),
      "{task}: the head's glyph is a shield with an alert"
    );
    assert_eq!(text(&mut window, &format!("{card}-came-text")), said, "{task}");
    assert_eq!(
      text(&mut window, &format!("{card}-reviewer-settings")),
      "Reviewer settings",
      "{task}"
    );
  }
}

/// An Autopilot step has no allowed list for MCP tools: its call's headline names the tool; a Manual step's call is
/// still not on its allowed list (ORC-586).
#[test]
fn card_approval_an_mcp_call_is_headed_by_its_mode() {
  for (task, title, reason) in [
    (
      "Redraw the retry banner",
      "Builder asks to call batch_design",
      "The reviewer wasn't sure: “Can't tell which files the design tool writes.”",
    ),
    (
      "Sketch the retry banner",
      "Builder asks to call a tool not on its allowed list",
      "Not on Builder's allowed list.",
    ),
  ] {
    let (mut window, ..) = opened_in(SCENARIO, task, "cards-approval-");
    let card = came_card(&mut window);
    assert_eq!(text(&mut window, &format!("{card}-title")), title, "{task}");
    assert_eq!(text(&mut window, &format!("{card}-came-text")), reason, "{task}");
  }
}

/// The card is compact, as 04c draws it: none of the full card's explanation, subject, command box or facts, and the
/// other answers only once *More answers* is pressed.
#[test]
fn card_approval_came_to_you_keeps_its_other_answers_under_more_answers() {
  let (mut window, ..) = opened_in(SCENARIO, "Publish the retry branch", "cards-approval-");
  let card = came_card(&mut window);
  assert_eq!(text(&mut window, &format!("{card}-action")), "git push origin HEAD");
  for part in [
    "-explain",
    "-subject",
    "-value",
    "-facts",
    "-deny-tells",
    "-wider",
    "-deny-message",
  ] {
    assert!(
      window.tree.get_element_by_id(&format!("{card}{part}")).is_none(),
      "{part} is not on the compact card"
    );
  }
  let more = format!("{card}-more-answers");
  assert_eq!(text(&mut window, &more), "More answers");
  click(&mut window, &more);
  window.until("the other answers", |window| {
    window.tree.get_element_by_id(&format!("{card}-wider")).is_some()
  });
  assert_eq!(
    text(&mut window, &format!("{card}-deny-message")),
    "Deny with a message"
  );
  assert!(window.tree.get_element_by_id(&format!("{card}-automation")).is_some());
}

#[test]
fn card_approval_rows_say_how_the_mode_approved_each_request() {
  let (mut window, ..) = opened_in(SCENARIO, "Cap Retry-After at 60 s", "cards-approval-");
  window.until("the two quiet rows", |window| leads(window).len() == 2);
  let mut said: Vec<String> = leads(&mut window).iter().map(|lead| text(&mut window, lead)).collect();
  said.sort();
  assert_eq!(said, ["Allowed by the profile", "Approved by the reviewer"]);
  let row = |window: &mut Window, lead: &str| {
    leads(window)
      .into_iter()
      .find(|id| text(window, id) == lead)
      .and_then(|id| id.strip_suffix("-lead").map(str::to_owned))
      .unwrap_or_else(|| panic!("the row {lead}"))
  };
  let reviewer = row(&mut window, "Approved by the reviewer");
  assert_eq!(
    text(&mut window, &format!("{reviewer}-asked")),
    "rm -rf target/fixtures-tmp"
  );
  let detail = text(&mut window, &format!("{reviewer}-detail"));
  assert!(
    detail.starts_with("· “Deletes generated fixtures inside the worktree.” · "),
    "{detail}"
  );
  let profile = row(&mut window, "Allowed by the profile");
  let detail = text(&mut window, &format!("{profile}-detail"));
  assert!(detail.starts_with("· on the allowed list · "), "{detail}");
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
