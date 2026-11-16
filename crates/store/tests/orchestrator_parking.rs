//! OA-15: the tasks the Orchestrator parked for the person, kept in the project database: one parking per task, read
//! in the order they were parked, ended once, and told as moved when the task's run changed or it has another.
//! `cargo test -p orchester-store --test orchestrator_parking parking`.

mod support;

use orchester_core::{
  id::{ChatId, TaskId},
  orchestrator::parking::{Needs, Parking, Words},
  run::{InterruptCause, Run},
  store::RunStore,
};
use support::{at, started_run};

fn parking(run: &Run, needs: Needs, when: i64) -> Parking {
  Parking {
    task: run.task(),
    run: Some(run.id()),
    needs,
    why: Words::new("Only you can sign in to GitHub.").expect("words"),
    to_do: Words::new("Sign in under Settings, then give the task back.").expect("words"),
    by: ChatId::generate(),
    at: at(when),
  }
}

#[test]
fn parking_is_kept_once_per_task_and_ended_once() {
  let (_repo, mut db, run, _execution) = started_run();
  assert_eq!(db.parked_task(run.task()).expect("read"), None);

  let first = parking(&run, Needs::SignIn, 100);
  db.park_task(&first).expect("parked");
  assert_eq!(db.parked_task(run.task()).expect("read"), Some(first));
  // Parked again, by another turn: the newer words take the place of the older.
  let second = parking(&run, Needs::Decision, 200);
  db.park_task(&second).expect("parked");
  assert_eq!(db.parked_tasks().expect("read"), vec![second.clone()]);

  assert_eq!(db.unpark_task(run.task()).expect("ended"), Some(second));
  assert_eq!(db.unpark_task(run.task()).expect("ended"), None, "ended once");
  assert!(db.parked_tasks().expect("read").is_empty());
}

#[test]
fn parking_is_told_as_moved_when_the_run_it_was_parked_with_changed() {
  let (_repo, mut db, mut run, _execution) = started_run();
  db.park_task(&parking(&run, Needs::Tool, 100)).expect("parked");
  assert!(
    db.parkings_moved().expect("read").is_empty(),
    "the run stands where it stood"
  );

  let events = run.interrupt(InterruptCause::Crashed, at(150)).expect("interrupted");
  db.append_events(&run, &events).expect("append");
  assert_eq!(db.parkings_moved().expect("read"), vec![run.task()]);
}

#[test]
fn parking_of_a_task_with_no_run_is_told_as_moved_once_it_has_one() {
  let (_repo, mut db, run, _execution) = started_run();
  // Parked as a task that never ran: the run it has is one it got since.
  let without_run = Parking {
    run: None,
    ..parking(&run, Needs::Other, 100)
  };
  db.park_task(&without_run).expect("parked");
  assert_eq!(db.parkings_moved().expect("read"), vec![run.task()]);

  // A task that has no run at all stays parked.
  let never_ran = Parking {
    task: TaskId::generate(),
    ..without_run
  };
  db.park_task(&never_ran).expect("parked");
  assert_eq!(
    db.parkings_moved().expect("read"),
    vec![run.task()],
    "only the one that got a run"
  );
}
