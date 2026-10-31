//! A decision model's verdict as data: approval at the threshold, the risk a card names, and the record it keeps
//! beside the language reviewers' ones (`docs/design/decision-reviewer.md`).

use orchester_core::run::{
  AnswerKind, PersonReason, ReviewQuestion, after_review,
  review::{DecisionVerdict, Probability, Review, ReviewEnd, Risk, Risks},
};

fn p(value: f64) -> Probability {
  Probability::new(value).expect("a probability")
}

fn risks([outside, network, credential, discards, off_task]: [f64; 5]) -> Risks {
  Risks {
    writes_outside_worktree: p(outside),
    reaches_network: p(network),
    reads_credential: p(credential),
    discards_work: p(discards),
    does_not_serve_task: p(off_task),
  }
}

fn verdict(approve: f64, threshold: f64) -> DecisionVerdict {
  DecisionVerdict {
    model: "jev".to_owned(),
    version: Some("2026-10".to_owned()),
    approve: p(approve),
    threshold: p(threshold),
    risks: risks([0.02, 0.1, 0.0, 0.78, 0.3]),
  }
}

#[test]
fn decision_verdict_approves_at_and_above_the_threshold() {
  for approve in [0.8, 0.97, 1.0] {
    let review = Review::Decided {
      decision: verdict(approve, 0.8),
    };
    assert!(review.approves(), "{approve}");
    assert_eq!(ReviewEnd::of(&review), Some(ReviewEnd::Approved));
    let answer = after_review(ReviewQuestion::Request, &review).expect("approved");
    assert_eq!(answer.kind, AnswerKind::Reviewer);
    assert_eq!(answer.reason, None);
  }
}

#[test]
fn decision_verdict_asks_below_the_threshold() {
  let review = Review::Decided {
    decision: verdict(0.41, 0.8),
  };
  assert!(!review.approves());
  assert_eq!(review.reason(), None);
  assert_eq!(ReviewEnd::of(&review), Some(ReviewEnd::Asked));
  let denied = after_review(ReviewQuestion::Request, &review).expect_err("to the person");
  assert_eq!(denied.reason, PersonReason::ReviewerDenied);
  let unsure = after_review(ReviewQuestion::Roots, &review).expect_err("to the person");
  assert_eq!(unsure.reason, PersonReason::ReviewerUnsure);
  assert!(
    !Review::Decided {
      decision: verdict(0.799, 0.8)
    }
    .approves()
  );
}

#[test]
fn decision_verdict_names_the_highest_risk_at_half_or_more() {
  assert_eq!(
    risks([0.02, 0.1, 0.0, 0.78, 0.3]).named(),
    Some((Risk::DiscardsWork, p(0.78)))
  );
  assert_eq!(
    risks([0.6, 0.9, 0.5, 0.7, 0.2]).named(),
    Some((Risk::ReachesNetwork, p(0.9)))
  );
  assert_eq!(
    risks([0.1, 0.0, 0.5, 0.2, 0.49]).named(),
    Some((Risk::ReadsCredential, p(0.5)))
  );
}

#[test]
fn decision_verdict_names_the_first_asked_of_tied_risks() {
  assert_eq!(
    risks([0.1, 0.7, 0.2, 0.7, 0.7]).named(),
    Some((Risk::ReachesNetwork, p(0.7)))
  );
  assert_eq!(
    risks([0.5, 0.5, 0.5, 0.5, 0.5]).named(),
    Some((Risk::WritesOutsideWorktree, p(0.5)))
  );
}

#[test]
fn decision_verdict_names_nothing_below_half() {
  assert_eq!(risks([0.49, 0.1, 0.0, 0.3, 0.499]).named(), None);
  assert_eq!(risks([0.0; 5]).named(), None);
}

#[test]
fn decision_verdict_refuses_numbers_that_are_not_probabilities() {
  for value in [-0.01, 1.01, f64::NAN, f64::INFINITY] {
    assert!(Probability::new(value).is_err(), "{value}");
  }
  let stored = r#"{"outcome":"decided","decision":{"model":"jev","approve":1.5,"threshold":0.8,"risks":{"writes_outside_worktree":0,"reaches_network":0,"reads_credential":0,"discards_work":0,"does_not_serve_task":0}}}"#;
  assert!(serde_json::from_str::<Review>(stored).is_err());
}

#[test]
fn decision_verdict_record_round_trips() {
  let review = Review::Decided {
    decision: verdict(0.41, 0.8),
  };
  let json = serde_json::to_value(&review).expect("serializes");
  assert_eq!(
    json,
    serde_json::json!({
      "outcome": "decided",
      "decision": {
        "model": "jev",
        "version": "2026-10",
        "approve": 0.41,
        "threshold": 0.8,
        "risks": {
          "writes_outside_worktree": 0.02,
          "reaches_network": 0.1,
          "reads_credential": 0.0,
          "discards_work": 0.78,
          "does_not_serve_task": 0.3,
        },
      },
    })
  );
  assert_eq!(serde_json::from_value::<Review>(json).expect("reads"), review);

  let unversioned = Review::Decided {
    decision: DecisionVerdict {
      version: None,
      ..verdict(0.97, 0.8)
    },
  };
  let text = serde_json::to_string(&unversioned).expect("serializes");
  assert!(!text.contains("version"), "{text}");
  assert_eq!(serde_json::from_str::<Review>(&text).expect("reads"), unversioned);
}

#[test]
fn decision_verdict_leaves_old_records_as_they_read() {
  let records = [
    (
      r#"{"outcome":"approved","reason":"It builds the crate.","model":"claude-haiku-4-5"}"#,
      Review::Approved {
        reason: "It builds the crate.".to_owned(),
        model: "claude-haiku-4-5".to_owned(),
      },
    ),
    (
      r#"{"outcome":"asked","reason":"It deletes a folder.","model":"claude-haiku-4-5"}"#,
      Review::Asked {
        reason: "It deletes a folder.".to_owned(),
        model: "claude-haiku-4-5".to_owned(),
      },
    ),
  ];
  for (stored, expected) in records {
    let review: Review = serde_json::from_str(stored).expect("an old record reads");
    assert_eq!(review, expected);
    assert_eq!(serde_json::to_string(&review).expect("serializes"), stored);
  }
}
