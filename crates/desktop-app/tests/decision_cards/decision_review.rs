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
    assert_eq!(text(&mut window, &format!("{card}-approve")), "Allow once");
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
      "Deny and tell Builder…"
    );
    assert_eq!(text(&mut window, &format!("{card}-more-answers")), "More");
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
    "Allow similar in this run"
  );
  assert_eq!(
    text(&mut window, &format!("{card}-everything")),
    "Allow all in this run"
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

#[test]
fn card_decision_review_shortcuts_only_answer_the_focused_card() {
  for digit in ["1", "2", "3", "4", "5"] {
    let (mut window, ..) = opened_in("cards-auto-mode", "Review two waiting commands", "cards-approval-");
    window.until("both waiting cards", |window| {
      super::cards(window, "cards-approval-")
        .iter()
        .filter(|id| id.ends_with("-approve"))
        .count()
        == 2
    });
    let mut cards: Vec<_> = super::cards(&mut window, "cards-approval-")
      .into_iter()
      .filter_map(|id| id.strip_suffix("-approve").map(str::to_owned))
      .collect();
    cards.sort();
    // Focus the later card: the earlier handler receives the key first.
    let (other, active) = (&cards[0], &cards[1]);
    let control = match digit {
      "1" => "approve",
      "2" => "similar",
      "3" => "everything",
      "4" => "deny-message",
      "5" => "more-answers",
      _ => unreachable!(),
    };
    window
      .tree
      .get_element_by_id_mut(&format!("{active}-{control}"))
      .expect("answer")
      .focus();
    window.pass();
    window.pass();
    lurq::app::events::with_synthetic_input(|| {
      window
        .tree
        .key_down(digit.to_owned(), format!("Digit{digit}"), false, false, false);
    });
    window.pass();
    window.pass();
    let suffix = match digit {
      // Approve once from synthetic input is refused to the client; neither card says anything of it.
      "1" => {
        assert!(
          window.refused_press().is_some(),
          "active {digit}: refused to the client"
        );
        assert!(window.tree.get_element_by_id(&format!("{active}-refused")).is_none());
        assert!(window.tree.get_element_by_id(&format!("{other}-refused")).is_none());
        continue;
      }
      "2" | "3" => "widen",
      "4" => "message",
      "5" => "automation",
      _ => unreachable!(),
    };
    assert!(
      window.tree.get_element_by_id(&format!("{active}-{suffix}")).is_some(),
      "active {digit}"
    );
    assert!(
      window.tree.get_element_by_id(&format!("{other}-{suffix}")).is_none(),
      "other {digit}"
    );
  }
}

#[test]
fn card_decision_review_shortcuts_leave_typing_in_the_composer_alone() {
  let (mut window, card, _) = opened_in("cards-auto-mode", "Merge the base branch", "cards-approval-");
  window.until("review answers", |window| {
    window.tree.get_element_by_id(&format!("{card}-similar")).is_some()
  });
  window.pass();
  window.pass();
  window
    .tree
    .get_element_by_id_mut("composer-input")
    .expect("composer")
    .focus();
  window.pass();
  assert_eq!(
    window.tree.focused_element().and_then(|element| element.id()),
    Some("composer-input")
  );
  for digit in ["1", "2", "3", "4", "5"] {
    window
      .tree
      .key_down(digit.to_owned(), format!("Digit{digit}"), false, false, false);
    window.pass();
    for suffix in ["refused", "widen", "message", "automation"] {
      assert!(
        window.tree.get_element_by_id(&format!("{card}-{suffix}")).is_none(),
        "{digit}: {suffix}"
      );
    }
    assert!(window.tree.get_element_by_id(&format!("{card}-approve")).is_some());
  }
}

#[test]
fn card_decision_review_denial_editor_keeps_controls_inside_the_compact_field() {
  let (mut window, card, _) = opened_in("cards-auto-mode", "Merge the base branch", "cards-approval-");
  window.until("review answers", |window| {
    window.tree.get_element_by_id(&format!("{card}-deny-message")).is_some()
  });
  click(&mut window, &format!("{card}-deny-message"));
  window.until("denial field", |window| {
    window.tree.get_element_by_id(&format!("{card}-message")).is_some()
  });
  assert_eq!(text(&mut window, &format!("{card}-message-back")), "Back");
  assert_eq!(text(&mut window, &format!("{card}-message-send")), "Deny and send");
  assert_eq!(
    text(&mut window, &format!("{card}-message-told")),
    "Builder is told this and goes on; Deny never fails the step"
  );
  for absent in ["message-label", "message-keys", "approve"] {
    assert!(window.tree.get_element_by_id(&format!("{card}-{absent}")).is_none());
  }
  let block = laid_out(&mut window, &format!("{card}-message-block"));
  let input = laid_out(&mut window, &format!("{card}-message"));
  let back = laid_out(&mut window, &format!("{card}-message-back"));
  let send = laid_out(&mut window, &format!("{card}-message-send"));
  assert!(block.height < 100.0, "one-line editor stays compact: {block:?}");
  assert!(input.y < back.y && back.y == send.y);
  assert!(send.y + send.height <= block.y + block.height);
  window
    .tree
    .key_down("Escape".to_owned(), "Escape".to_owned(), false, false, false);
  window.pass();
  window.pass();
  assert!(window.tree.get_element_by_id(&format!("{card}-message")).is_none());
}

#[test]
fn card_decision_review_denial_enter_sends_but_preserves_the_person_only_guard() {
  let (mut window, card, _) = opened_in("cards-auto-mode", "Merge the base branch", "cards-approval-");
  window.until("review answers", |window| {
    window.tree.get_element_by_id(&format!("{card}-deny-message")).is_some()
  });
  click(&mut window, &format!("{card}-deny-message"));
  window.until("denial field", |window| {
    window.tree.get_element_by_id(&format!("{card}-message")).is_some()
  });
  let words = "Keep the reviewed branch.";
  window
    .tree
    .get_element_by_id_mut(&format!("{card}-message"))
    .and_then(|element| element.as_text_input())
    .expect("message field")
    .set_value(words);
  window.pass();
  window.pass();
  lurq::app::events::with_synthetic_input(|| {
    window
      .tree
      .key_down("Enter".to_owned(), "Enter".to_owned(), false, false, false);
  });
  assert!(window.refused_press().is_some(), "refused to the client");
  window.pass();
  assert!(
    window.tree.get_element_by_id(&format!("{card}-refused")).is_none(),
    "the card says nothing of it"
  );
  assert_eq!(
    window.texts(&format!("{card}-message")).unwrap_or_default().join(""),
    words
  );
  window
    .tree
    .key_down("Enter".to_owned(), "Enter".to_owned(), false, false, false);
  window.pass();
  window.until("runner refusal", |window| {
    window.tree.get_element_by_id(&format!("{card}-refused")).is_some()
  });
}
