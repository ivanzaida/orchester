//! Decision review lines of 05 JFEPS, including the tool approval path.
use super::{opened_in, text};

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
    assert_eq!(text(&mut window, &format!("{card}-approve")), "Approve");
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
