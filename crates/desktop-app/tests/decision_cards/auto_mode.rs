//! A command as auto mode's review leaves it, on the `cards-auto-mode` scenario: while the reviewer judges it, a quiet
//! line without answers until the person chooses *Answer myself*; left to the person (the reviewer asked, or its
//! review ran out of time), the whole card with its answers; approved by the reviewer, the decided row.

use super::{click, opened_in, text};

const SCENARIO: &str = "cards-auto-mode";

#[test]
fn card_auto_mode_under_review_is_a_quiet_line_until_the_person_answers_it() {
  let (mut window, card, _) = opened_in(SCENARIO, "Show the changelog's last lines", "cards-approval-");
  assert_eq!(
    text(&mut window, &format!("{card}-checking")),
    "Auto mode is checking this command…"
  );
  assert_eq!(text(&mut window, &format!("{card}-asked")), "tail -n 3 CHANGELOG.md");
  for answer in ["-approve", "-deny", "-reviewing"] {
    assert!(
      window.tree.get_element_by_id(&format!("{card}{answer}")).is_none(),
      "no {answer} while auto mode reviews it"
    );
  }

  click(&mut window, &format!("{card}-answer-myself"));
  window.until("the whole card", |window| {
    window.tree.get_element_by_id(&format!("{card}-approve")).is_some()
  });
  assert_eq!(
    text(&mut window, &format!("{card}-reviewing")),
    "Auto mode is reviewing it; you can still answer."
  );
  assert!(window.tree.get_element_by_id(&format!("{card}-checking")).is_none());
}

#[test]
fn card_auto_mode_left_to_the_person_shows_the_whole_card() {
  // The reviewer asked the person, and a review that ran out of time asks them too.
  for title in ["Bump the HTTP client", "List the duplicate crates"] {
    let (mut window, card, _) = opened_in(SCENARIO, title, "cards-approval-");
    window.until(&format!("{title}: the answers"), |window| {
      window.tree.get_element_by_id(&format!("{card}-approve")).is_some()
    });
    let shown = text(&mut window, &card);
    assert!(shown.contains("Approve command?"), "{title}: {shown}");
    assert!(!shown.contains("Auto mode is reviewing"), "{title}: {shown}");
    assert!(
      window.tree.get_element_by_id(&format!("{card}-checking")).is_none(),
      "{title}"
    );
  }
}

#[test]
fn card_auto_mode_approved_reads_approved_by_auto_mode() {
  let (mut window, card, _) = opened_in(SCENARIO, "Read the webhook test log", "cards-approval-");
  let said = text(&mut window, &format!("{card}-decided"));
  assert!(said.starts_with("Approved by auto mode at "), "{said}");
  assert_eq!(
    text(&mut window, &format!("{card}-asked")),
    "tail -n 40 target/webhook-tests.log"
  );
  assert!(window.tree.get_element_by_id(&format!("{card}-answer-myself")).is_none());
}
