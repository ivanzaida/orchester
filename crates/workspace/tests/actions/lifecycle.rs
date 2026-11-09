//! A team task's workflow change (decision D4: Done, Canceled, reopening, Backlog and Todo) is the task's patch, whose
//! response can be lost and whose revision can have moved on: a lost response is read again and the change made once,
//! a task another person already moved to where the change goes is that change made, one moved elsewhere comes back
//! with its current value; and the server's lifecycle reasons are the refusals the screens name.

use orchester_api::{ErrorCode, SharingErrorDetails};
use orchester_core::task::organization::{TaskOrganization, WorkflowStatus};
use orchester_workspace::actions::{
  Accepted, Action, Current, Definition, Intent, LifecycleRefusal, Refusal, TaskAction, TeamOutcome,
};

use crate::{
  Checkout, revision,
  server::{ADMIN, TeamServer},
  team,
};

/// The task a completed change holds.
fn made(outcome: TeamOutcome) -> Box<orchester_api::SharedTask> {
  match outcome {
    TeamOutcome::Completed(Accepted::Definition(Definition::Task(task))) => task,
    outcome => panic!("not a made task: {outcome:?}"),
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_a_lost_response_of_a_close_is_read_again_and_made_once() {
  let server = TeamServer::start();
  let task = server.add_task("Sign in");
  server.move_to(task.id, "todo");
  let task = server.task(task.id);
  let checkout = Checkout::linked(&server);
  let (dispatcher, _generation) = checkout.team(&server, ADMIN);
  let base = TaskOrganization::default().with_status(WorkflowStatus::Todo);
  let intent = Intent::new(Action::Task(TaskAction::Organize {
    task: task.id,
    read: revision(task.revision),
    edited: base.clone().with_status(WorkflowStatus::Done),
    base,
  }));

  server.lose_next_response();
  let done = made(team(dispatcher.dispatch(&intent).await.expect("dispatched")));
  assert_eq!(done.content.workflow, WorkflowStatus::Done);
  assert_eq!(server.writes(), 1, "made once");

  // The same intent again: the conflict's current task holds the change, so it was made.
  let again = made(team(dispatcher.dispatch(&intent).await.expect("dispatched")));
  assert_eq!(again.content.workflow, WorkflowStatus::Done);
  assert_eq!(server.writes(), 1, "and not made a second time");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_a_close_over_a_task_that_moved_on_comes_with_its_current_value() {
  let server = TeamServer::start();
  let task = server.add_task("Sign in");
  server.move_to(task.id, "todo");
  let checkout = Checkout::linked(&server);
  let (dispatcher, _generation) = checkout.team(&server, ADMIN);
  // Another Admin cancels it after it was read at revision 2.
  let read = server.task(task.id).revision;
  server.move_to(task.id, "canceled");
  let base = TaskOrganization::default().with_status(WorkflowStatus::Todo);
  let intent = Intent::new(Action::Task(TaskAction::Organize {
    task: task.id,
    read: revision(read),
    edited: base.clone().with_status(WorkflowStatus::Done),
    base,
  }));

  let refused = team(dispatcher.dispatch(&intent).await.expect("dispatched"));
  let TeamOutcome::Rejected {
    refusal: Refusal::Route { status, code },
    retryable: false,
    current: Some(Current::Task(current)),
    ..
  } = refused
  else {
    panic!("not a conflict with the current task: {refused:?}");
  };
  assert_eq!((status, code), (409, ErrorCode::RevisionConflict));
  assert_eq!(current.content.workflow, WorkflowStatus::Canceled);
  assert_eq!(server.writes(), 0, "nothing was written");

  // A task another Admin already closed the same way is that change made: no refusal.
  server.move_to(task.id, "done");
  let done = made(team(dispatcher.dispatch(&intent).await.expect("dispatched")));
  assert_eq!(done.content.workflow, WorkflowStatus::Done);
  assert_eq!(server.writes(), 0, "the server's own state already held it");
}

#[test]
fn actions_the_wire_names_of_a_lifecycle_refusal_are_the_screens_refusals() {
  let reason = |reason: &str| SharingErrorDetails::Reason {
    reason: reason.to_owned(),
  };
  let of = |status, code, details: Option<&SharingErrorDetails>| Refusal::of_task_route(status, code, details);
  assert!(matches!(
    of(422, ErrorCode::Organization, Some(&reason("stop_run_first"))),
    Refusal::TaskLifecycle(LifecycleRefusal::StopRunFirst)
  ));
  assert!(matches!(
    of(422, ErrorCode::Organization, Some(&reason("missing_reason"))),
    Refusal::TaskLifecycle(LifecycleRefusal::MissingReason)
  ));
  assert!(matches!(
    of(409, ErrorCode::NotHolder, None),
    Refusal::TaskLifecycle(LifecycleRefusal::NotHolder)
  ));
  // A reason the app does not name, and other codes, stay the route's status and code.
  assert!(matches!(
    of(422, ErrorCode::Organization, Some(&reason("anything_else"))),
    Refusal::Route {
      status: 422,
      code: ErrorCode::Organization
    }
  ));
  assert!(matches!(
    of(403, ErrorCode::NotAdmin, None),
    Refusal::Route {
      status: 403,
      code: ErrorCode::NotAdmin
    }
  ));
}
