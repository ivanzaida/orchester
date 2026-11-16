//! What a team task's capabilities let a person do with its lifecycle (decision D4): the actions the server lists and
//! allows, and whether it lists any at all, since a screen states who does the rest only where the server listed the
//! lifecycle (a rule it never denied is not one to tell).
//!
//! `cargo test -p orchester-app --test team_lifecycle_abilities team_lifecycle_abilities`

use orchester_api::v2::{Action, ActionCapability, CallerRole, Capabilities, DenialReason, LifecycleAction, Subject};
use orchester_app::team::pages::lifecycle;
use orchester_core::{
  id::{ProjectId, TaskId},
  sharing::TaskRevision,
};

fn capabilities(actions: Vec<ActionCapability>) -> Capabilities {
  Capabilities {
    project_id: ProjectId::generate(),
    revision: TaskRevision::FIRST,
    role: CallerRole::Member,
    actions,
    unresolved: Vec::new(),
  }
}

fn task() -> Subject {
  Subject::Task { id: TaskId::generate() }
}

#[test]
fn team_lifecycle_abilities_a_task_that_lists_none_tells_no_rule() {
  let subject = task();
  let read = capabilities(vec![ActionCapability::allowed(Action::Read, None)]);
  let abilities = lifecycle(&read, subject);
  assert!(!abilities.listed, "no lifecycle action is listed");
  assert!(!abilities.complete && !abilities.cancel && !abilities.reopen && !abilities.triage && !abilities.block);
}

#[test]
fn team_lifecycle_abilities_a_denied_action_is_listed_and_not_allowed() {
  let subject = task();
  let read = capabilities(vec![
    ActionCapability::denied(
      Action::Task(LifecycleAction::Cancel),
      Some(subject),
      DenialReason::RoleRequired,
    ),
    ActionCapability::allowed(Action::Task(LifecycleAction::Complete), Some(subject)),
  ]);
  let abilities = lifecycle(&read, subject);
  assert!(abilities.listed);
  assert!(abilities.complete, "the allowed action");
  assert!(!abilities.cancel, "the denied one is listed and not offered");
}

#[test]
fn team_lifecycle_abilities_another_tasks_actions_are_not_this_ones() {
  let (subject, other) = (task(), task());
  let read = capabilities(vec![ActionCapability::allowed(
    Action::Task(LifecycleAction::Complete),
    Some(other),
  )]);
  let abilities = lifecycle(&read, subject);
  assert!(!abilities.listed && !abilities.complete);
}
