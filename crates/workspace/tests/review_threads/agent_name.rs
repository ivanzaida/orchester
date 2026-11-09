//! The agent of the implementing step as the review page's threads name it (ORC-963): the step and its agent, but the
//! step's name once when the agent's name already begins with it.
//! `cargo test -p orchester-workspace --test review_threads agent_name`.

use orchester_workspace::tasks::review_threads::page::agent_name;

#[test]
fn review_threads_agent_name_joins_the_step_and_an_agent_that_does_not_begin_with_it() {
  assert_eq!(agent_name("Implement", Some("Builder")), ("Implement", Some("Builder")));
}

#[test]
fn review_threads_agent_name_says_the_step_once_when_the_agent_begins_with_it() {
  assert_eq!(
    agent_name("Implement", Some("Implement · Claude 2 · fast")),
    ("Implement · Claude 2 · fast", None)
  );
  assert_eq!(agent_name("Implement", Some("implement")), ("implement", None));
  assert_eq!(
    agent_name("Implement", Some("Implement-fast")),
    ("Implement-fast", None)
  );
}

#[test]
fn review_threads_agent_name_keeps_both_when_the_agent_only_starts_with_the_same_letters() {
  assert_eq!(
    agent_name("Implement", Some("Implementer")),
    ("Implement", Some("Implementer"))
  );
  assert_eq!(agent_name("Review", Some("Re")), ("Review", Some("Re")));
}

#[test]
fn review_threads_agent_name_is_the_step_alone_while_the_agent_is_gone() {
  assert_eq!(agent_name("Implement", None), ("Implement", None));
}
