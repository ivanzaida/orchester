//! A QA copy's MCP client counts as the person for every input it sends the window (`orchester_app::mcp::qa_person`,
//! the person's decision of 2026-11-15): until the QA copy's program says so, automation's input is refused where
//! only a person may press and window input is refused while a person-only control shows; afterwards neither is. The
//! switch is the program's, so this file is a test program of its own, with one test.
//! `cargo test -p orchester-app --test qa_every_input qa_every_input`.

use lurq::app::events::with_synthetic_input;
use orchester_app::mcp::{
  InputGate,
  input_gate::{Guarded, persons_press},
  input_source, is_synthetic_input,
  qa_person::{count_every_input_as_the_persons, counts_every_input},
};

#[test]
fn qa_every_input_is_the_persons_only_once_the_qa_copy_says_so() {
  let gate = InputGate::default();
  let _accounts = gate.protect(Guarded::Accounts);

  // As the installed app and a normal start have it.
  assert!(!counts_every_input());
  assert!(!is_synthetic_input(), "a person's input is never automation's");
  assert!(with_synthetic_input(is_synthetic_input));
  assert!(
    gate.protecting(),
    "window input is refused while Settings › Accounts shows"
  );
  assert!(!with_synthetic_input(|| persons_press(Some(&gate), "add an account")));
  assert!(
    gate.take_refused_press().is_some(),
    "refused to the client that pressed"
  );

  count_every_input_as_the_persons();

  assert!(
    !with_synthetic_input(is_synthetic_input),
    "the client's input is the person's"
  );
  assert_eq!(with_synthetic_input(input_source), lurq::app::events::InputSource::Os);
  assert!(!gate.protecting(), "nothing in the window is kept from it");
  assert!(with_synthetic_input(|| persons_press(Some(&gate), "add an account")));
  assert!(gate.take_refused_press().is_none());
}
