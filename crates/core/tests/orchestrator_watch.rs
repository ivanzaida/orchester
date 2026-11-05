//! The subscription rules of a leading conversation (ORC-753): a module's or a task's subscription ends by itself once
//! its tasks are Done or Canceled, with the reason for its summary turn, while tasks without a module and the project
//! never do; a person's stop lists what it left open and changes no run; a task the conversation creates outside its
//! targets becomes its task subscription with the order of the turn's subscription.

use orchester_core::{
  id::{ModuleId, TaskId},
  orchestrator::watch::{
    EndReason, Ending, LeftOpen, NeverEnds, OpenWork, TargetTasks, Watch, WatchTarget, created_task_watch,
  },
  run::TaskPhase,
  task::organization::WorkflowStatus::{self, Backlog, Canceled, Done, InProgress, InReview, Todo},
};

#[test]
fn watch_ending_a_task_ends_once_done_or_canceled() {
  assert_eq!(TargetTasks::Task(Done).ending(), Ending::Ends(EndReason::TaskDone));
  assert_eq!(
    TargetTasks::Task(Canceled).ending(),
    Ending::Ends(EndReason::TaskCanceled)
  );
  for open in [Backlog, Todo, InProgress, InReview] {
    assert_eq!(TargetTasks::Task(open).ending(), Ending::Open, "{open:?} keeps it");
  }
}

#[test]
fn watch_ending_a_module_ends_once_every_task_is_done_or_canceled() {
  assert_eq!(
    TargetTasks::Module(&[Done, Canceled, Done]).ending(),
    Ending::Ends(EndReason::ModuleClosed { done: 2, canceled: 1 })
  );
  assert_eq!(
    TargetTasks::Module(&[Canceled]).ending(),
    Ending::Ends(EndReason::ModuleClosed { done: 0, canceled: 1 })
  );
  assert_eq!(TargetTasks::Module(&[Done, InReview, Done]).ending(), Ending::Open);
  let none: &[WorkflowStatus] = &[];
  assert_eq!(
    TargetTasks::Module(none).ending(),
    Ending::Open,
    "a module without tasks goes on"
  );
}

#[test]
fn watch_ending_never_for_tasks_without_a_module_or_the_project() {
  assert_eq!(
    TargetTasks::WithoutModule.ending(),
    Ending::Never(NeverEnds::WithoutModule)
  );
  assert_eq!(TargetTasks::Project.ending(), Ending::Never(NeverEnds::Project));
}

#[test]
fn watch_stop_lists_what_was_left_open_and_changes_no_run() {
  let [waits, stuck, runs, queued, at_limit, paused, merged, draft] = [(); 8].map(|()| TaskId::generate());
  let phases = [
    (waits, TaskPhase::Waiting),
    (stuck, TaskPhase::Stuck),
    (runs, TaskPhase::Running),
    (queued, TaskPhase::Queued),
    (at_limit, TaskPhase::StoppedAtLimit),
    (paused, TaskPhase::Paused),
    (merged, TaskPhase::Merged),
    (draft, TaskPhase::Draft),
  ];
  let left = LeftOpen::after_stop(phases.iter().copied());
  assert_eq!(
    left.tasks(),
    [
      (waits, OpenWork::Waits),
      (stuck, OpenWork::Waits),
      (runs, OpenWork::Runs),
      (queued, OpenWork::Queued),
      (at_limit, OpenWork::StoppedAtLimit),
    ]
  );
  assert_eq!(left.count(OpenWork::Waits), 2);
  assert_eq!(left.count(OpenWork::StoppedAtLimit), 1);
  assert_eq!(
    phases[0],
    (waits, TaskPhase::Waiting),
    "a stop reads the phases and changes none"
  );
  assert!(LeftOpen::after_stop([(merged, TaskPhase::Merged)]).is_empty());
}

fn watch(target: WatchTarget, order: Option<&str>) -> Watch {
  Watch {
    target,
    order: order.map(str::to_owned),
  }
}

#[test]
fn watch_created_task_outside_the_targets_becomes_a_task_subscription_with_the_turns_order() {
  let module = ModuleId::generate();
  let led = watch(WatchTarget::Module(module), Some("Merge when green"));
  let watches = [led.clone(), watch(WatchTarget::Task(TaskId::generate()), None)];
  let created = TaskId::generate();
  assert_eq!(
    created_task_watch(&watches, Some(&led), created, None),
    Some(watch(WatchTarget::Task(created), Some("Merge when green")))
  );
  assert_eq!(
    created_task_watch(&watches, None, created, Some(ModuleId::generate())),
    Some(watch(WatchTarget::Task(created), None)),
    "a turn the person started gives the default order"
  );
}

#[test]
fn watch_created_task_inside_a_target_adds_nothing() {
  let module = ModuleId::generate();
  let task = TaskId::generate();
  let by_module = [watch(WatchTarget::Module(module), Some("Ask first"))];
  assert_eq!(
    created_task_watch(&by_module, Some(&by_module[0]), task, Some(module)),
    None
  );
  let by_task = [watch(WatchTarget::Task(task), None)];
  assert_eq!(created_task_watch(&by_task, None, task, None), None);
  let without_module = [watch(WatchTarget::WithoutModule, None)];
  assert_eq!(created_task_watch(&without_module, None, task, None), None);
  assert!(created_task_watch(&without_module, None, task, Some(module)).is_some());
  let project = [watch(WatchTarget::Project, None)];
  assert_eq!(created_task_watch(&project, None, task, Some(module)), None);
}
