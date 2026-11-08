//! What the team server refused a version with, read back from the code kept on it (`sharing::team_edit::Refusal`):
//! the refusals a person can act on, and any other by its code.
//!
//! `cargo test -p orchester-workspace --test team_refusal`

use orchester_workspace::sharing::team_edit::{Refusal, Rejection};

#[test]
fn team_refusal_reads_the_codes_a_person_can_act_on() {
  assert_eq!(Refusal::of_code("not_admin"), Refusal::NotAdmin);
  assert_eq!(Refusal::of_code("slug_taken"), Refusal::NameTaken);
  assert_eq!(Refusal::of_code("version_conflict"), Refusal::Changed);
  assert_eq!(Refusal::of_code("archived"), Refusal::Rejected(Rejection::Archived));
}

#[test]
fn team_refusal_reads_the_reason_of_an_invalid_content() {
  assert_eq!(Refusal::of_code("invalid:body"), Refusal::Rejected(Rejection::Body));
  assert_eq!(
    Refusal::of_code("invalid:references"),
    Refusal::Rejected(Rejection::References)
  );
  assert_eq!(Refusal::of_code("invalid:shape"), Refusal::Rejected(Rejection::Shape));
}

#[test]
fn team_refusal_names_any_other_code_and_never_guesses() {
  assert_eq!(Refusal::of_code("invalid"), Refusal::Other("invalid".to_owned()));
  assert_eq!(
    Refusal::of_code("rate_limited"),
    Refusal::Other("rate_limited".to_owned())
  );
  assert_eq!(
    Refusal::of_code("invalid:catalog"),
    Refusal::Other("invalid:catalog".to_owned())
  );
}
