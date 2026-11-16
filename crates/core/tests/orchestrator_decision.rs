//! OA-22: the reason of a decision the Orchestrator made on its own, and how the person reviews it.
//! `cargo test -p orchester-core --test orchestrator_decision decision`.

use orchester_core::{
  orchestrator::decision::{MAX_REASON, Reason, Review, Undo},
  time::Timestamp,
};

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

#[test]
fn decision_reason_is_one_line_that_says_something() {
  assert_eq!(
    Reason::new("  The review passed;\n the comment is cosmetic. ")
      .expect("a reason")
      .as_str(),
    "The review passed; the comment is cosmetic."
  );
  assert_eq!(Reason::new(""), None);
  assert_eq!(Reason::new(" \n\t "), None, "blanks say nothing");
  assert!(Reason::new(&"a".repeat(MAX_REASON)).is_some());
  assert_eq!(
    Reason::new(&"a".repeat(MAX_REASON + 1)),
    None,
    "a sentence, not a report"
  );
}

#[test]
fn decision_review_goes_forward_only() {
  let (seen, overruled) = (Review::Seen { at: at(10) }, Review::Overruled { at: at(20) });
  assert!(Review::Open.is_open());
  assert!(Review::Open.may_become(seen));
  assert!(Review::Open.may_become(overruled));
  assert!(
    seen.may_become(overruled),
    "the person may still overrule what they saw"
  );
  assert!(!seen.may_become(Review::Open));
  assert!(!seen.may_become(Review::Seen { at: at(30) }));
  assert!(!overruled.may_become(seen), "an overruling is not taken back");
  assert!(!overruled.may_become(Review::Open));
  assert!(!Review::Open.may_become(Review::Open));
}

#[test]
fn decision_undo_is_stored_by_its_name() {
  for undo in [Undo::ReturnToStep, Undo::GiveBack] {
    assert_eq!(Undo::named(undo.name()), Some(undo));
  }
  assert_eq!(Undo::named("revert_merge"), None, "nothing else is promised");
}
