use orchester_core::{
  id::{ProjectId, RunId, TaskId},
  run::{
    BranchName, RunWaitsOn, WaitReason, WaitedTaskEvent, WaitedTaskEventKind, WaitingRun, WaitsOnChange, WaitsOnEnd,
    WaitsOnError,
  },
  task::organization::WorkflowStatus,
};

fn branch(name: &str) -> BranchName {
  BranchName::new(name).expect("a valid branch name")
}

fn waiting(project: ProjectId, waits_for: WaitReason) -> WaitingRun {
  WaitingRun {
    project,
    run: RunId::generate(),
    task: TaskId::generate(),
    base: branch("master"),
    waits_for,
  }
}

fn wait() -> RunWaitsOn {
  let project = ProjectId::generate();
  RunWaitsOn::new(waiting(project, WaitReason::Message), TaskId::generate(), project).expect("a valid wait")
}

fn event(wait: &RunWaitsOn, kind: WaitedTaskEventKind) -> Option<WaitsOnEnd> {
  Some(WaitsOnEnd::Event(WaitedTaskEvent {
    task: wait.task(),
    run: wait.run(),
    waited: wait.waited(),
    kind,
  }))
}

#[test]
fn waits_on_a_run_may_wait_on_another_task_of_its_project_while_it_waits_for_a_message_or_a_decision() {
  let project = ProjectId::generate();
  for reason in [WaitReason::Message, WaitReason::Plan] {
    let run = waiting(project, reason);
    let waited = TaskId::generate();
    let wait = RunWaitsOn::new(run.clone(), waited, project).expect("a valid wait");
    assert_eq!(
      (wait.run(), wait.task(), wait.base(), wait.waited()),
      (run.run, run.task, &run.base, waited)
    );
  }
}

#[test]
fn waits_on_a_run_cannot_wait_on_its_own_task_another_projects_or_while_a_tool_waits_for_approval() {
  let project = ProjectId::generate();
  let run = waiting(project, WaitReason::Message);
  assert_eq!(
    RunWaitsOn::new(run.clone(), run.task, project),
    Err(WaitsOnError::OwnTask)
  );
  assert_eq!(
    RunWaitsOn::new(run, TaskId::generate(), ProjectId::generate()),
    Err(WaitsOnError::OtherProject)
  );
  assert_eq!(
    RunWaitsOn::new(waiting(project, WaitReason::Approval), TaskId::generate(), project),
    Err(WaitsOnError::NotWaitingForPerson)
  );
}

#[test]
fn waits_on_a_run_waits_on_one_task_at_a_time() {
  let project = ProjectId::generate();
  let run = waiting(project, WaitReason::Message);
  let first = RunWaitsOn::new(run.clone(), TaskId::generate(), project).expect("a valid wait");
  let second = RunWaitsOn::new(run, TaskId::generate(), project).expect("a valid wait");
  let other = wait();
  assert_eq!(first.check_only(&[]), Ok(()));
  assert_eq!(
    second.check_only(std::slice::from_ref(&first)),
    Err(WaitsOnError::AlreadyWaits(first.waited()))
  );
  assert_eq!(other.check_only(&[first]), Ok(()));
}

#[test]
fn waits_on_the_waited_tasks_merge_into_the_runs_base_is_an_event_for_the_waiting_task() {
  let wait = wait();
  let merged = WaitsOnChange::Merged {
    task: wait.waited(),
    branch: branch("master"),
  };
  let end = wait.end(&merged);
  assert_eq!(
    end,
    event(
      &wait,
      WaitedTaskEventKind::Merged {
        branch: branch("master")
      }
    )
  );
  let Some(WaitsOnEnd::Event(event)) = end else {
    unreachable!("asserted above")
  };
  assert!(!event.kind.tells_person());
}

#[test]
fn waits_on_a_merge_into_another_branch_or_of_another_task_does_not_end_the_wait() {
  let wait = wait();
  let elsewhere = WaitsOnChange::Merged {
    task: wait.waited(),
    branch: branch("release"),
  };
  let other = WaitsOnChange::Merged {
    task: TaskId::generate(),
    branch: branch("master"),
  };
  assert_eq!(wait.end(&elsewhere), None);
  assert_eq!(wait.end(&other), None);
}

#[test]
fn waits_on_the_waited_task_canceled_or_closed_without_a_merge_tells_the_person() {
  let wait = wait();
  for (status, kind) in [
    (WorkflowStatus::Canceled, WaitedTaskEventKind::Canceled),
    (WorkflowStatus::Done, WaitedTaskEventKind::ClosedUnmerged),
  ] {
    let end = wait.end(&WaitsOnChange::StatusChanged {
      task: wait.waited(),
      status,
    });
    assert!(kind.tells_person());
    assert_eq!(end, event(&wait, kind));
  }
  for status in [
    WorkflowStatus::Backlog,
    WorkflowStatus::Todo,
    WorkflowStatus::InProgress,
    WorkflowStatus::InReview,
  ] {
    let change = WaitsOnChange::StatusChanged {
      task: wait.waited(),
      status,
    };
    assert_eq!(wait.end(&change), None);
  }
  let other = WaitsOnChange::StatusChanged {
    task: TaskId::generate(),
    status: WorkflowStatus::Canceled,
  };
  assert_eq!(wait.end(&other), None);
}

#[test]
fn waits_on_the_waiting_run_ended_drops_the_wait_silently() {
  let wait = wait();
  assert_eq!(
    wait.end(&WaitsOnChange::RunEnded { run: wait.run() }),
    Some(WaitsOnEnd::Dropped)
  );
  assert_eq!(wait.end(&WaitsOnChange::RunEnded { run: RunId::generate() }), None);
}
