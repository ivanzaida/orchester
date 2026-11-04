//! A team project's definitions through the dispatcher: a task edit goes to `PATCH /api/v1/projects/{p}/tasks/{t}`
//! and the workspace policy to `PATCH /api/v2/projects/{p}/policy`, never to the commands. A lost response is read
//! again and the edit made once; the same intent sent again finds it made. A revision conflict comes with the current
//! value, and a member's edit is refused and changes nothing.

use orchester_api::{
  ErrorCode,
  v2::{AuthorityErrorCode, PolicyRevision},
};
use orchester_core::store::TaskStore;
use orchester_workspace::actions::{Accepted, Current, Definition, Intent, Refusal, TeamOutcome};

use crate::{
  Checkout, create, policy, retitle,
  server::{ADMIN, MEMBER, TeamServer},
  team,
};

/// The definition a completed edit holds.
fn made(outcome: TeamOutcome) -> Definition {
  match outcome {
    TeamOutcome::Completed(Accepted::Definition(definition)) => definition,
    outcome => panic!("not a made definition: {outcome:?}"),
  }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_a_lost_definition_response_is_read_again_and_made_once() {
  let server = TeamServer::start();
  let task = server.add_task("Sign in");
  let checkout = Checkout::linked(&server);
  let (dispatcher, _generation) = checkout.team(&server, ADMIN);
  server.lose_next_response();
  let intent = Intent::new(retitle(task.id, 1, "Sign in", "Log in"));
  let Definition::Task(edited) = made(team(dispatcher.dispatch(&intent).await.expect("dispatched"))) else {
    panic!("not a task");
  };
  assert_eq!((edited.content.title.as_str(), edited.revision.get()), ("Log in", 2));
  assert_eq!(server.writes(), 1, "made once");
  assert_eq!(server.counts(), (0, 0), "no command");
  assert_eq!(
    checkout.db().team_operation(intent.operation()).expect("read"),
    None,
    "a definition edit keeps no operation"
  );

  // The same intent again: the conflict's current task holds the edit, so it was made.
  let again = made(team(dispatcher.dispatch(&intent).await.expect("dispatched")));
  assert!(matches!(again, Definition::Task(task) if task.revision.get() == 2));
  assert_eq!(server.writes(), 1);

  server.lose_next_response();
  let changed = made(team(
    dispatcher
      .dispatch(&Intent::new(policy(1, true)))
      .await
      .expect("dispatched"),
  ));
  let Definition::Policy(changed) = changed else {
    panic!("not the policy: {changed:?}");
  };
  assert_eq!((changed.revision.get(), changed.automatic_merge), (2, true));
  assert_eq!((server.writes(), server.policy()), (2, (2, true)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_a_revision_conflict_returns_the_current_value() {
  let server = TeamServer::start();
  let task = server.add_task("Sign in");
  server.retitle(task.id, "Sign in with a passkey");
  let checkout = Checkout::linked(&server);
  let (dispatcher, _generation) = checkout.team(&server, ADMIN);
  let refused = team(
    dispatcher
      .dispatch(&Intent::new(retitle(task.id, 1, "Sign in", "Log in")))
      .await
      .expect("dispatched"),
  );
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
  assert_eq!(
    (current.content.title.as_str(), current.revision.get()),
    ("Sign in with a passkey", 2)
  );

  server.set_policy(true);
  let refused = team(
    dispatcher
      .dispatch(&Intent::new(policy(1, false)))
      .await
      .expect("dispatched"),
  );
  let TeamOutcome::Rejected {
    refusal: Refusal::PolicyRevisionConflict,
    current: Some(Current::Policy(current)),
    ..
  } = refused
  else {
    panic!("not a conflict with the current policy: {refused:?}");
  };
  assert_eq!(current.revision, PolicyRevision::try_from(2).expect("revision"));
  assert!(current.automatic_merge);
  assert_eq!(server.writes(), 0);
  assert!(dispatcher.open_operations().expect("open").is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_member_writes_are_rejected_and_change_nothing_here() {
  let server = TeamServer::start();
  let task = server.add_task("Sign in");
  let checkout = Checkout::linked(&server);
  let (dispatcher, _generation) = checkout.team(&server, MEMBER);
  let created = team(
    dispatcher
      .dispatch(&Intent::new(create("Sign out")))
      .await
      .expect("dispatched"),
  );
  assert!(
    matches!(&created, TeamOutcome::Rejected { refusal: Refusal::Command(error), retryable: false, .. }
      if error.code() == AuthorityErrorCode::Forbidden),
    "{created:?}"
  );
  let edited = team(
    dispatcher
      .dispatch(&Intent::new(retitle(task.id, 1, "Sign in", "Log in")))
      .await
      .expect("dispatched"),
  );
  assert!(
    matches!(
      edited,
      TeamOutcome::Rejected {
        refusal: Refusal::Route {
          status: 403,
          code: ErrorCode::NotAdmin
        },
        retryable: false,
        ..
      }
    ),
    "{edited:?}"
  );
  let changed = team(
    dispatcher
      .dispatch(&Intent::new(policy(1, true)))
      .await
      .expect("dispatched"),
  );
  assert!(
    matches!(
      changed,
      TeamOutcome::Rejected {
        refusal: Refusal::Route { status: 403, .. },
        ..
      }
    ),
    "{changed:?}"
  );
  assert_eq!(server.task(task.id).title, "Sign in");
  assert_eq!(
    (server.counts().1, server.writes(), server.policy()),
    (0, 0, (1, false))
  );
  let db = checkout.db();
  assert_eq!(db.task_identity(task.id).expect("read"), None, "nothing recorded here");
  assert!(db.tasks().expect("tasks").is_empty(), "no local task instead");
  assert!(dispatcher.open_operations().expect("open").is_empty());
}
