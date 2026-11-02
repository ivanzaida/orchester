//! Decision review lines of 05 JFEPS, including the tool approval path.
use super::{click, laid_out, opened_in, text};

#[test]
fn card_decision_review_names_probability_threshold_and_risk() {
  for task in ["Restore the reviewed files", "Review the tool request"] {
    let (mut window, card, _) = opened_in("cards-auto-mode", task, "cards-approval-");
    window.until("decision review", |window| {
      window.tree.get_element_by_id(&format!("{card}-review")).is_some()
    });
    assert_eq!(text(&mut window, &format!("{card}-review-model")), "jev left it to you");
    assert_eq!(text(&mut window, &format!("{card}-review-probability")), "0.41");
    assert_eq!(text(&mut window, &format!("{card}-review-threshold")), "under 0.80 ·");
    assert_eq!(
      text(&mut window, &format!("{card}-review-risk")),
      "may discard work in the worktree"
    );
    assert_eq!(text(&mut window, &format!("{card}-review-risk-probability")), "0.78");
    assert!(window.tree.get_element_by_id(&format!("{card}-came")).is_none());
    assert_eq!(text(&mut window, &format!("{card}-approve")), "Allow once 1");
  }
}

#[test]
fn card_decision_review_omits_risks_below_half() {
  let (mut window, card, _) = opened_in("cards-auto-mode", "Merge the base branch", "cards-approval-");
  window.until("decision review", |window| {
    window.tree.get_element_by_id(&format!("{card}-review")).is_some()
  });
  assert_eq!(text(&mut window, &format!("{card}-review-probability")), "0.72");
  assert_eq!(
    text(&mut window, &format!("{card}-review-threshold")),
    "under 0.80 · no risk above 0.5"
  );
  assert!(window.tree.get_element_by_id(&format!("{card}-review-risk")).is_none());
}

#[test]
fn card_decision_review_failure_names_the_model_and_reason_without_figures() {
  let (mut window, card, _) = opened_in("cards-auto-mode", "Check the translations", "cards-approval-");
  window.until("decision review", |window| {
    window.tree.get_element_by_id(&format!("{card}-review")).is_some()
  });
  assert_eq!(
    text(&mut window, &format!("{card}-review-failure")),
    "jev did not answer in time, so it came to you"
  );
  assert_eq!(
    text(&mut window, &format!("{card}-review-settings")),
    "· Project settings › Connections"
  );
  assert!(
    window
      .tree
      .get_element_by_id(&format!("{card}-review-probability"))
      .is_none()
  );
}

#[test]
fn card_decision_review_shell_keeps_question_answers_and_footer_in_design_order() {
  for task in [
    "Restore the reviewed files",
    "Merge the base branch",
    "Check the translations",
    "Review the tool request",
  ] {
    let (mut window, card, _) = opened_in("cards-auto-mode", task, "cards-approval-");
    window.until("decision review", |window| {
      window.tree.get_element_by_id(&format!("{card}-review")).is_some()
    });
    let expected = if task == "Review the tool request" {
      "Allow Builder to call this tool?"
    } else {
      "Allow Builder to run this command?"
    };
    assert_eq!(text(&mut window, &format!("{card}-question")), expected);
    let command = laid_out(&mut window, &format!("{card}-command"));
    let review = laid_out(&mut window, &format!("{card}-review"));
    let question = laid_out(&mut window, &format!("{card}-question"));
    let allow = laid_out(&mut window, &format!("{card}-approve"));
    let deny = laid_out(&mut window, &format!("{card}-deny-message"));
    let meta = laid_out(&mut window, &format!("{card}-meta"));
    assert!(command.y < review.y && review.y < question.y && question.y < allow.y);
    assert!(allow.y < deny.y && deny.y < meta.y);
    assert_eq!(
      text(&mut window, &format!("{card}-deny-message")),
      "Deny and tell Builder… 4"
    );
    assert_eq!(text(&mut window, &format!("{card}-more-answers")), "More 5");
    for suffix in ["copy", "reply", "fold"] {
      assert!(
        window.tree.get_element_by_id(&format!("{card}-{suffix}")).is_some(),
        "{suffix}"
      );
    }
    click(&mut window, &format!("{card}-fold"));
    assert!(window.tree.get_element_by_id(&format!("{card}-approve")).is_none());
    click(&mut window, &card);
    assert!(window.tree.get_element_by_id(&format!("{card}-approve")).is_some());
    // Reopening a row grows it below its folded position; scroll its answers back into the chat viewport.
    let chat = laid_out(&mut window, "conversation-chat");
    let (x, y) = (chat.x + chat.width / 2.0, chat.y + chat.height / 2.0);
    window
      .tree
      .scroll(x, y, 0.0, -1000.0, lurq::app::events::ScrollPhase::Scroll);
    window.pass();
    window.pass();
    click(&mut window, &format!("{card}-deny-message"));
    window.until(&format!("{task}: denial editor"), |window| {
      window.tree.get_element_by_id(&format!("{card}-message")).is_some()
    });
    assert!(window.tree.get_element_by_id(&format!("{card}-approve")).is_none());
  }
}

#[test]
fn card_decision_review_wider_answer_keeps_confirmation_and_automation_guard() {
  let (mut window, card, _) = opened_in("cards-auto-mode", "Merge the base branch", "cards-approval-");
  window.until("decision review", |window| {
    window.tree.get_element_by_id(&format!("{card}-review")).is_some()
  });
  assert_eq!(
    text(&mut window, &format!("{card}-similar")),
    "Allow similar in this run 2"
  );
  assert_eq!(
    text(&mut window, &format!("{card}-everything")),
    "Allow all in this run 3"
  );
  click(&mut window, &format!("{card}-similar"));
  assert!(window.tree.get_element_by_id(&format!("{card}-widen")).is_some());
  assert!(window.tree.get_element_by_id(&format!("{card}-refused")).is_none());
  assert!(text(&mut window, &format!("{card}-consequence")).contains("Run"));
  window.pass();
  lurq::app::events::with_synthetic_input(|| click(&mut window, &format!("{card}-widen-confirm")));
  assert!(window.tree.get_element_by_id(&format!("{card}-widen")).is_some());
  assert!(window.tree.get_element_by_id(&format!("{card}-refused")).is_none());
}
