//! Why auto mode left a request to the person, in words (ORC-80): every hard rule of `run::review::ReviewHold` has a
//! sentence of its own in the workspace's locale, which the approval card and the MCP reads give.
//! `cargo test -p orchester-workspace --test approval_holds approval_holds`.

use std::collections::HashSet;

use orchester_core::run::review::ReviewHold;
use orchester_workspace::{
  i18n::{FALLBACK_LOCALE, LOCALES},
  tasks::approvals::hold_cause,
};
use serde_json::Value;

const HOLDS: [ReviewHold; 12] = [
  ReviewHold::Secrets,
  ReviewHold::Unreadable,
  ReviewHold::Destructive,
  ReviewHold::Push,
  ReviewHold::ReadOnlyStep,
  ReviewHold::NetworkOff,
  ReviewHold::OutsideFolders,
  ReviewHold::PermissionsUnknown,
  ReviewHold::TooLong,
  ReviewHold::Escalation,
  ReviewHold::ProjectProgram,
  ReviewHold::Browser,
];

/// The words of `hold` in the fallback locale.
fn words(locale: &Value, hold: ReviewHold) -> String {
  let message = hold_cause(hold);
  assert!(message.args.is_empty(), "{hold:?} names no value");
  message
    .key
    .split('.')
    .fold(locale, |value, part| &value[part])
    .as_str()
    .unwrap_or_else(|| panic!("{} is in {FALLBACK_LOCALE}.json", message.key))
    .to_owned()
}

#[test]
fn approval_holds_each_say_their_rule_in_plain_words() {
  let (_, contents) = LOCALES
    .iter()
    .find(|(locale, _)| *locale == FALLBACK_LOCALE)
    .expect("the fallback locale is shipped");
  let locale: Value = serde_json::from_str(contents).expect("the locale is JSON");
  let mut said = HashSet::new();
  for hold in HOLDS {
    let text = words(&locale, hold);
    assert!(text.starts_with("Auto mode didn’t review this: "), "{hold:?}: {text}");
    assert!(
      said.insert(text.clone()),
      "{hold:?} says what another rule says: {text}"
    );
  }
  assert_eq!(
    words(&locale, ReviewHold::ReadOnlyStep),
    "Auto mode didn’t review this: the step only reads the repository, and this command may write."
  );
}
