//! The states of tasks the team server records as closed or blocked (decision D4, `Task - v2` `jDN0r` T4 `V26A3`, T5
//! `eJKwy`, T6 `Xwa1j`, T11a–f): `team-closed-admin` and `team-closed-member` are Kontur with one task of each way a
//! task is closed or blocked, as the stand-in team server's own rules and records make them, never seeded: someone
//! else's patch, the accepted delivery of a module, a work's reported success. KON-1, Done in the server's tasks
//! without a record, is the task whose closing the app does not know. Fixture data for QA, never shipped.

use orchester_api::test_support::team_server::{AuthorityClaim, AuthorityTask, Person, TeamServer};
use orchester_core::resource::Deliverable;

use super::State;

const MARA: Person = Person::Named("mara");
const DANA: Person = Person::Named("dana-k");

/// Done by the person (T11d `dTn0v`): KON-9, and by mara (T4 `V26A3`): KON-8.
const DONE_BY_YOU: u32 = 9;
const DONE_BY_MARA: u32 = 8;
/// Canceled by dana-k, an Admin (T11e `b3t84e`, T11f `vyQJx`).
const CANCELED: u32 = 15;
/// Blocked by mara, who holds it (T11c `x0xP1`), and by the person, who holds it (T11a `ujJ6U`).
const BLOCKED_BY_MARA: u32 = 16;
const BLOCKED_BY_YOU: u32 = 21;
/// Done with the delivery of Webhooks v2 (T5 `eJKwy`): in that module, its work succeeded.
const DELIVERED: u32 = 19;
/// Done by its run (T6 `Xwa1j`): its work reports and delivers no changes.
const REPORTED: u32 = 20;
/// The index of Webhooks v2 among the modules.
const WEBHOOKS: usize = 0;

const REASON: &str = "Waiting for the staging key";

/// Whether `state` is one of these.
pub const fn is(state: State) -> bool {
  matches!(state, State::ClosedAdmin | State::ClosedMember)
}

const fn claim(holder: Person) -> Option<AuthorityClaim> {
  Some(AuthorityClaim {
    holder,
    renewed_minutes_ago: 3,
    stale: false,
    generation: 0,
  })
}

/// `task` as these states have it before the records: the claims the tasks that are blocked or closed by themselves
/// hold, and the open workflow of KON-15, which the server's tasks have Done.
pub fn task(task: &mut AuthorityTask) {
  let (holder, work) = match task.number {
    BLOCKED_BY_MARA => (MARA, "running"),
    BLOCKED_BY_YOU => (Person::You, "running"),
    DELIVERED | REPORTED => (MARA, "succeeded"),
    CANCELED => {
      task.workflow = "in_progress";
      return;
    }
    _ => return,
  };
  task.claim = claim(holder);
  task.work = Some(work);
  task.assignee = Some(holder);
}

/// What happens on the server once its tasks are served.
pub fn records(server: &TeamServer) {
  server.task_closed_by(DONE_BY_YOU, Person::You, "done");
  server.task_closed_by(DONE_BY_MARA, MARA, "done");
  server.task_closed_by(CANCELED, DANA, "canceled");
  server.task_blocked_by(BLOCKED_BY_MARA, MARA, REASON);
  server.task_blocked_by(BLOCKED_BY_YOU, Person::You, REASON);
  server.module_delivered(WEBHOOKS);
  server.authority_delivers(REPORTED, Deliverable::Report);
  server.work_succeeded(REPORTED);
}
