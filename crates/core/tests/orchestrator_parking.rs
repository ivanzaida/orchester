//! OA-15: the words and the kinds of a task parked for the person.
//! `cargo test -p orchester-core --test orchestrator_parking parking`.

use orchester_core::orchestrator::parking::{MAX_WORDS, Needs, Words};

#[test]
fn parking_words_are_one_line_that_says_something() {
  assert_eq!(
    Words::new(" Sign in to GitHub\n in Settings. ")
      .expect("words")
      .as_str(),
    "Sign in to GitHub in Settings."
  );
  assert_eq!(Words::new("  \n "), None);
  assert!(Words::new(&"a".repeat(MAX_WORDS)).is_some());
  assert_eq!(Words::new(&"a".repeat(MAX_WORDS + 1)), None);
}

#[test]
fn parking_needs_are_stored_by_their_names() {
  for needs in Needs::ALL {
    assert_eq!(Needs::named(needs.name()), Some(needs));
  }
  assert_eq!(Needs::named("money"), None);
}
