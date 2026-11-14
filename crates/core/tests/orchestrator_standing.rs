//! OA-12: where a led target stands, for every state of a task and of the whole, and the edge cases of the task file.
//! `cargo test -p orchester-core --test orchestrator_standing standing`.

use orchester_core::{
  orchestrator::{
    Ceilings,
    standing::{Around, Parked, TargetStanding, TaskFacts, TaskStanding, Unready, standing},
  },
  run::TaskPhase,
};

fn task(key: &str, phase: TaskPhase) -> TaskFacts {
  TaskFacts {
    key: key.to_owned(),
    closed: false,
    phase,
    runs: usize::from(!matches!(phase, TaskPhase::Draft | TaskPhase::Queued)),
    unready: None,
    parked: None,
    blockers: Vec::new(),
    low_disk: false,
    held: false,
  }
}

fn blocked(key: &str, phase: TaskPhase, by: &[&str]) -> TaskFacts {
  TaskFacts {
    blockers: by.iter().map(|key| (*key).to_owned()).collect(),
    ..task(key, phase)
  }
}

fn leading() -> Around {
  Around {
    free_slots: 2,
    ceilings: Some(Ceilings::default()),
    budget_used: false,
    delivery: None,
  }
}

fn of(facts: &TaskFacts) -> TaskStanding {
  facts.standing(leading().ceilings)
}

#[test]
fn standing_of_a_task_follows_its_latest_run() {
  assert_eq!(of(&task("A", TaskPhase::Running)), TaskStanding::Working);
  assert_eq!(of(&task("A", TaskPhase::Interrupted)), TaskStanding::Working);
  assert_eq!(
    of(&task("A", TaskPhase::Queued)),
    TaskStanding::Working,
    "queued to start"
  );
  assert_eq!(of(&task("A", TaskPhase::Waiting)), TaskStanding::WaitsForPerson);
  assert_eq!(of(&task("A", TaskPhase::Stuck)), TaskStanding::WaitsForPerson);
  assert_eq!(of(&task("A", TaskPhase::StoppedAtLimit)), TaskStanding::WaitsForLimit);
  assert_eq!(of(&task("A", TaskPhase::Paused)), TaskStanding::Paused);
  let low_disk = TaskFacts {
    low_disk: true,
    ..task("A", TaskPhase::Paused)
  };
  assert_eq!(of(&low_disk), TaskStanding::WaitsForDisk);
  assert_eq!(of(&task("A", TaskPhase::Succeeded)), TaskStanding::ToMerge);
  assert_eq!(of(&task("A", TaskPhase::Merged)), TaskStanding::Done);
  assert_eq!(of(&task("A", TaskPhase::Cancelled)), TaskStanding::Canceled);
  assert_eq!(of(&task("A", TaskPhase::Draft)), TaskStanding::ReadyToStart);
  let closed = TaskFacts {
    closed: true,
    ..task("A", TaskPhase::Failed)
  };
  assert_eq!(of(&closed), TaskStanding::Done, "Done or Canceled, whatever its run");
}

#[test]
fn standing_of_a_failed_task_says_whether_it_may_run_again() {
  let failed = |runs| TaskFacts {
    runs,
    ..task("A", TaskPhase::Failed)
  };
  let allowed = usize::from(Ceilings::default().runs_of_task());
  assert_eq!(of(&failed(1)), TaskStanding::Failed { runs: 1, again: true });
  assert_eq!(
    of(&failed(allowed)),
    TaskStanding::Failed {
      runs: allowed,
      again: false
    },
    "past its ceiling of runs"
  );
  assert_eq!(
    failed(1).standing(None),
    TaskStanding::Failed { runs: 1, again: false },
    "only followed: nothing runs it again"
  );
}

#[test]
fn standing_of_a_task_with_no_run_names_what_keeps_it_from_starting() {
  let unready = TaskFacts {
    unready: Some(Unready::NoPipeline),
    blockers: vec!["B".to_owned()],
    held: true,
    ..task("A", TaskPhase::Draft)
  };
  assert_eq!(
    of(&unready),
    TaskStanding::NotReady(Unready::NoPipeline),
    "the person's first"
  );
  let not_ready = TaskFacts {
    unready: Some(Unready::PipelineNotReady),
    ..task("A", TaskPhase::Draft)
  };
  assert_eq!(of(&not_ready), TaskStanding::NotReady(Unready::PipelineNotReady));
  assert_eq!(
    of(&blocked("A", TaskPhase::Draft, &["B", "C"])),
    TaskStanding::BlockedBy(vec!["B".to_owned(), "C".to_owned()])
  );
  assert_eq!(
    of(&blocked("A", TaskPhase::Queued, &["B"])),
    TaskStanding::BlockedBy(vec!["B".to_owned()]),
    "queued, and held by a blocking task"
  );
  let held = TaskFacts {
    held: true,
    ..task("A", TaskPhase::Draft)
  };
  assert_eq!(of(&held), TaskStanding::Held);
  for parked in [Parked::ByPerson, Parked::ByChat] {
    let left = TaskFacts {
      parked: Some(parked),
      ..task("A", TaskPhase::Waiting)
    };
    assert_eq!(
      of(&left),
      TaskStanding::Parked(parked),
      "left to the person, whatever its run"
    );
  }
}

#[test]
fn standing_of_a_target_is_done_when_every_task_is() {
  assert_eq!(
    standing(&[], &[], &leading()).target,
    TargetStanding::Done,
    "nothing to do"
  );
  let done = [task("A", TaskPhase::Merged), task("B", TaskPhase::Merged)];
  assert_eq!(standing(&done, &[], &leading()).target, TargetStanding::Done);
}

#[test]
fn standing_of_a_target_names_what_can_start_and_whether_a_slot_is_free() {
  let failed_again = TaskFacts {
    runs: 1,
    ..task("C", TaskPhase::Failed)
  };
  let tasks = [
    task("A", TaskPhase::Running),
    task("B", TaskPhase::Draft),
    failed_again,
    blocked("D", TaskPhase::Draft, &["B"]),
  ];
  let read = standing(&tasks, &[], &leading());
  assert_eq!(
    read.target,
    TargetStanding::CanStart {
      tasks: vec!["B".to_owned(), "C".to_owned()],
      slot_free: true
    }
  );
  assert_eq!(
    read.tasks[3],
    ("D".to_owned(), TaskStanding::BlockedBy(vec!["B".to_owned()]))
  );
  let full = Around {
    free_slots: 0,
    ..leading()
  };
  assert!(
    matches!(
      standing(&tasks, &[], &full).target,
      TargetStanding::CanStart { slot_free: false, .. }
    ),
    "it can start once an agent is free"
  );
}

#[test]
fn standing_of_a_target_works_while_something_moves_and_is_stuck_when_nothing_does() {
  let parked = TaskFacts {
    parked: Some(Parked::ByChat),
    ..task("P", TaskPhase::Waiting)
  };
  let moving = [task("A", TaskPhase::Waiting), parked.clone()];
  assert_eq!(standing(&moving, &[], &leading()).target, TargetStanding::Working);

  let past_ceiling = TaskFacts {
    runs: usize::from(Ceilings::default().runs_of_task()),
    ..task("F", TaskPhase::Failed)
  };
  let left = [
    parked,
    past_ceiling,
    blocked("W", TaskPhase::Draft, &["P"]),
    task("X", TaskPhase::Paused),
    task("M", TaskPhase::Merged),
  ];
  assert_eq!(
    standing(&left, &[], &leading()).target,
    TargetStanding::Stuck { budget_used: false },
    "all that is left is parked, failed past its ceiling, blocked by something parked, or paused by the person"
  );
}

#[test]
fn standing_of_a_target_whose_budget_is_used_starts_nothing() {
  let spent = Around {
    budget_used: true,
    ..leading()
  };
  let ready = [task("A", TaskPhase::Draft)];
  assert_eq!(
    standing(&ready, &[], &spent).target,
    TargetStanding::Stuck { budget_used: true }
  );
  let still_working = [task("A", TaskPhase::Draft), task("B", TaskPhase::Running)];
  assert_eq!(standing(&still_working, &[], &spent).target, TargetStanding::Working);
}

#[test]
fn standing_of_a_task_blocked_from_outside_moves_only_while_its_blocker_does() {
  let waiting = [blocked("A", TaskPhase::Draft, &["OUT-1"])];
  let working_outside = [task("OUT-1", TaskPhase::Running)];
  assert_eq!(
    standing(&waiting, &working_outside, &leading()).target,
    TargetStanding::Working
  );

  // A task outside that could start is nobody's to start from here: what waits on it does not move.
  let ready_outside = [task("OUT-1", TaskPhase::Draft)];
  assert_eq!(
    standing(&waiting, &ready_outside, &leading()).target,
    TargetStanding::Stuck { budget_used: false }
  );
  assert_eq!(
    standing(&waiting, &[], &leading()).target,
    TargetStanding::Stuck { budget_used: false },
    "a blocker nobody told of will not move"
  );

  // A chain: A waits on B, B on OUT-1, which works.
  let chain = [
    blocked("A", TaskPhase::Draft, &["B"]),
    blocked("B", TaskPhase::Queued, &["OUT-1"]),
  ];
  assert_eq!(
    standing(&chain, &working_outside, &leading()).target,
    TargetStanding::Working
  );
  // Two tasks that block each other move nothing.
  let knot = [
    blocked("A", TaskPhase::Draft, &["B"]),
    blocked("B", TaskPhase::Draft, &["A"]),
  ];
  assert_eq!(
    standing(&knot, &[], &leading()).target,
    TargetStanding::Stuck { budget_used: false }
  );
}

#[test]
fn standing_names_are_stable() {
  assert_eq!(TaskStanding::Parked(Parked::ByChat).name(), "parked");
  assert_eq!(TaskStanding::Parked(Parked::ByPerson).name(), "parked_by_person");
  assert_eq!(TaskStanding::NotReady(Unready::NoPipeline).name(), "no_pipeline");
  assert_eq!(TargetStanding::Stuck { budget_used: false }.name(), "stuck");
  assert_eq!(
    TargetStanding::CanStart {
      tasks: Vec::new(),
      slot_free: false
    }
    .name(),
    "can_start"
  );
}
