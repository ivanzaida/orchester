//! The rules of `orchestrator::watch`. Which Orchestrator conversation leads a task's events (ORC-752): the most
//! specific subscription, one leader per target, the task inside a module another conversation leads, taking a target
//! over and stopping. When a subscription ends and what the leading conversation creates (ORC-753): a module's or a
//! task's subscription ends by itself once its tasks are Done or Canceled, with the reason for its summary turn, while
//! tasks without a module and the project never do; a person's stop lists what it left open and changes no run; a
//! task the conversation creates outside its targets becomes its task subscription with the order of the turn's
//! subscription.

use orchester_core::{
  id::{ChatId, ModuleId, TaskId},
  orchestrator::{
    Claim, Subscription, Subscriptions, WatchTarget,
    watch::{
      Created, EndReason, Ending, LeftOpen, NeverEnds, News, OpenWork, TargetTasks, TwoLeaders, created_task,
      created_task_watch, event_subscription,
    },
  },
  run::TaskPhase,
  task::organization::WorkflowStatus::{self, Backlog, Canceled, Done, InProgress, InReview, Todo},
  time::Timestamp,
};

fn at(seconds: i64) -> Timestamp {
  Timestamp::from_unix_seconds(seconds)
}

fn watch(chat: ChatId, target: WatchTarget, since: i64) -> Subscription {
  Subscription::new(chat, target, "", at(since))
}

fn set(kept: impl IntoIterator<Item = Subscription>) -> Subscriptions {
  Subscriptions::new(kept).expect("one leader per target")
}

#[test]
fn watch_leading_takes_the_most_specific_subscription() {
  let (project, module_chat, task_chat) = (ChatId::generate(), ChatId::generate(), ChatId::generate());
  let (task, other, module) = (TaskId::generate(), TaskId::generate(), ModuleId::generate());
  let all = set([
    watch(project, WatchTarget::Project, 1),
    watch(module_chat, WatchTarget::Module(module), 2),
    watch(task_chat, WatchTarget::Task(task), 3),
  ]);
  assert_eq!(
    all.leading(task, Some(module)),
    Some(task_chat),
    "the task's own subscription first"
  );
  assert_eq!(all.leading(other, Some(module)), Some(module_chat), "then its module's");
  assert_eq!(
    all.leading(other, Some(ModuleId::generate())),
    Some(project),
    "then the project's"
  );
  assert_eq!(
    all.leading(other, None),
    Some(project),
    "a task without a module, when nobody watches those"
  );
}

#[test]
fn watch_leading_of_tasks_without_a_module() {
  let (project, loose) = (ChatId::generate(), ChatId::generate());
  let task = TaskId::generate();
  let all = set([
    watch(project, WatchTarget::Project, 1),
    watch(loose, WatchTarget::WithoutModule, 2),
  ]);
  assert_eq!(all.leading(task, None), Some(loose));
  assert_eq!(
    all.leading(task, Some(ModuleId::generate())),
    Some(project),
    "a task in a module is not one of them"
  );
}

#[test]
fn watch_leading_is_none_without_a_covering_subscription() {
  let task = TaskId::generate();
  assert_eq!(Subscriptions::default().leading(task, None), None);
  let elsewhere = set([
    watch(ChatId::generate(), WatchTarget::Task(TaskId::generate()), 1),
    watch(ChatId::generate(), WatchTarget::Module(ModuleId::generate()), 2),
  ]);
  assert_eq!(
    elsewhere.leading(task, Some(ModuleId::generate())),
    None,
    "callers keep today's rule"
  );
}

#[test]
fn watch_set_refuses_two_leaders_of_one_target() {
  let module = WatchTarget::Module(ModuleId::generate());
  let refused = Subscriptions::new([
    watch(ChatId::generate(), module, 1),
    watch(ChatId::generate(), module, 2),
  ]);
  assert_eq!(refused, Err(TwoLeaders { target: module }));
  let chat = ChatId::generate();
  let same_chat = Subscriptions::new([
    watch(chat, WatchTarget::Project, 1),
    watch(chat, WatchTarget::Project, 2),
  ]);
  assert_eq!(
    same_chat,
    Err(TwoLeaders {
      target: WatchTarget::Project
    })
  );
}

#[test]
fn watch_claim_new_here_or_led_elsewhere() {
  let (mine, theirs) = (ChatId::generate(), ChatId::generate());
  let module = WatchTarget::Module(ModuleId::generate());
  let all = set([watch(mine, WatchTarget::WithoutModule, 1), watch(theirs, module, 5)]);
  assert_eq!(all.claim(mine, WatchTarget::WithoutModule, None), Claim::AlreadyHere);
  assert_eq!(
    all.claim(mine, module, None),
    Claim::LedElsewhere {
      chat: theirs,
      since: at(5)
    }
  );
  assert_eq!(
    all.claim(mine, WatchTarget::Project, None),
    Claim::New { takes_from: None }
  );
}

#[test]
fn watch_claim_of_a_task_inside_a_module_led_elsewhere_is_new() {
  let (mine, module_chat, project) = (ChatId::generate(), ChatId::generate(), ChatId::generate());
  let (task, module) = (TaskId::generate(), ModuleId::generate());
  let all = set([
    watch(project, WatchTarget::Project, 1),
    watch(module_chat, WatchTarget::Module(module), 2),
  ]);
  assert_eq!(
    all.claim(mine, WatchTarget::Task(task), Some(module)),
    Claim::New {
      takes_from: Some(module_chat)
    },
    "not a conflict: the module's conversation loses the task"
  );
  assert_eq!(
    all.claim(mine, WatchTarget::Task(task), None),
    Claim::New {
      takes_from: Some(project)
    },
    "a task without a module comes from the project's"
  );
  assert_eq!(
    all.claim(module_chat, WatchTarget::Task(task), Some(module)),
    Claim::New { takes_from: None }
  );

  let taken = all.take_over(
    Subscription::new(mine, WatchTarget::Task(task), "merge it", at(9)),
    Some(module),
  );
  let told = taken.told().expect("the module's conversation is told");
  assert_eq!(told.chat(), module_chat);
  assert_eq!(
    told.news(),
    News::Loses {
      target: WatchTarget::Task(task),
      to: mine
    }
  );
  assert_eq!(taken.set().leading(task, Some(module)), Some(mine));
  assert_eq!(
    taken.set().leading(TaskId::generate(), Some(module)),
    Some(module_chat),
    "its other tasks stay there"
  );
}

#[test]
fn watch_take_over_moves_the_target_and_tells_its_leader() {
  let (mine, theirs) = (ChatId::generate(), ChatId::generate());
  let (task, module) = (TaskId::generate(), ModuleId::generate());
  let all = set([
    watch(theirs, WatchTarget::Module(module), 1),
    watch(mine, WatchTarget::Project, 2),
  ]);
  let taken = all.take_over(Subscription::new(mine, WatchTarget::Module(module), "  ", at(7)), None);
  let told = taken.told().expect("the other conversation is told");
  assert_eq!(told.chat(), theirs);
  assert_eq!(
    told.news(),
    News::TakenOver {
      target: WatchTarget::Module(module),
      by: mine
    }
  );
  let led = taken.set().of(WatchTarget::Module(module)).expect("led");
  assert_eq!((led.chat(), led.since(), led.order()), (mine, at(7), None));
  assert_eq!(taken.set().of_chat(theirs).count(), 0);
  assert_eq!(taken.set().leading(task, Some(module)), Some(mine));
  assert_eq!(taken.set().as_slice().len(), 2, "still one leader per target");
}

#[test]
fn watch_take_over_of_its_own_target_replaces_the_order_quietly() {
  let chat = ChatId::generate();
  let all = set([watch(chat, WatchTarget::Project, 1)]);
  let taken = all.take_over(
    Subscription::new(chat, WatchTarget::Project, "only report", at(4)),
    None,
  );
  assert_eq!(taken.told(), None);
  assert_eq!(
    taken.set().of(WatchTarget::Project).and_then(Subscription::order),
    Some("only report")
  );
  assert_eq!(taken.into_set().as_slice().len(), 1);
}

#[test]
fn watch_stop_ends_the_subscription_and_hands_the_tasks_back() {
  let (mine, module_chat) = (ChatId::generate(), ChatId::generate());
  let (task, module) = (TaskId::generate(), ModuleId::generate());
  let all = set([
    watch(module_chat, WatchTarget::Module(module), 1),
    watch(mine, WatchTarget::Task(task), 2),
  ]);
  assert_eq!(
    all.stop(module_chat, WatchTarget::Task(task), Some(module)),
    None,
    "only its leader stops it"
  );

  let stopped = all.stop(mine, WatchTarget::Task(task), Some(module)).expect("stopped");
  assert_eq!(stopped.stopped().target(), WatchTarget::Task(task));
  let told = stopped.told().expect("the module's conversation is told");
  assert_eq!(told.chat(), module_chat);
  assert_eq!(
    told.news(),
    News::Regains {
      target: WatchTarget::Task(task),
      from: mine
    }
  );
  assert_eq!(stopped.set().leading(task, Some(module)), Some(module_chat));

  let last = stopped
    .into_set()
    .stop(module_chat, WatchTarget::Module(module), None)
    .expect("stopped");
  assert_eq!(last.told(), None, "nobody else watches the project");
  assert_eq!(last.set().leading(task, Some(module)), None);
}

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

#[test]
fn watch_created_task_outside_the_targets_becomes_a_task_subscription_with_the_turns_order() {
  let (chat, other) = (ChatId::generate(), ChatId::generate());
  let module = ModuleId::generate();
  let led = Subscription::new(chat, WatchTarget::Module(module), "Merge when green", at(1));
  let all = set([
    led.clone(),
    watch(chat, WatchTarget::Task(TaskId::generate()), 2),
    watch(other, WatchTarget::Project, 3),
  ]);
  let created = TaskId::generate();
  assert_eq!(
    created_task_watch(&all, chat, Some(&led), created, None, at(9)),
    Some(Subscription::new(
      chat,
      WatchTarget::Task(created),
      "Merge when green",
      at(9)
    )),
    "another conversation's subscription does not take it in for this one"
  );
  assert_eq!(
    created_task_watch(&all, chat, None, created, Some(ModuleId::generate()), at(9)),
    Some(watch(chat, WatchTarget::Task(created), 9)),
    "a turn the person started gives the default order"
  );
}

#[test]
fn watch_created_task_inside_a_target_adds_nothing() {
  let chat = ChatId::generate();
  let module = ModuleId::generate();
  let task = TaskId::generate();
  let by_module = Subscription::new(chat, WatchTarget::Module(module), "Ask first", at(1));
  assert_eq!(
    created_task_watch(
      &set([by_module.clone()]),
      chat,
      Some(&by_module),
      task,
      Some(module),
      at(9)
    ),
    None
  );
  let by_task = set([watch(chat, WatchTarget::Task(task), 1)]);
  assert_eq!(created_task_watch(&by_task, chat, None, task, None, at(9)), None);
  let without_module = set([watch(chat, WatchTarget::WithoutModule, 1)]);
  assert_eq!(created_task_watch(&without_module, chat, None, task, None, at(9)), None);
  assert!(created_task_watch(&without_module, chat, None, task, Some(module), at(9)).is_some());
  let project = set([watch(chat, WatchTarget::Project, 1)]);
  assert_eq!(
    created_task_watch(&project, chat, None, task, Some(module), at(9)),
    None
  );
}

#[test]
fn watch_created_task_led_elsewhere_or_by_a_conversation_that_watches_nothing_is_not_taken() {
  let (chat, billing_chat, loose_chat) = (ChatId::generate(), ChatId::generate(), ChatId::generate());
  let (delivery, billing) = (ModuleId::generate(), ModuleId::generate());
  let billing_watch = watch(billing_chat, WatchTarget::Module(billing), 2);
  let loose_watch = watch(loose_chat, WatchTarget::WithoutModule, 3);
  let all = set([
    watch(chat, WatchTarget::Module(delivery), 1),
    billing_watch.clone(),
    loose_watch.clone(),
  ]);
  let task = TaskId::generate();
  assert_eq!(
    created_task(&all, chat, None, (task, Some(billing)), at(9)),
    Created::LedElsewhere(billing_watch)
  );
  assert_eq!(
    created_task(&all, chat, None, (task, None), at(9)),
    Created::LedElsewhere(loose_watch)
  );
  assert_eq!(
    created_task(&all, ChatId::generate(), None, (task, None), at(9)),
    Created::Unchanged,
    "a conversation that watches nothing"
  );
}

#[test]
fn watch_event_subscription_is_the_conversations_most_specific_one() {
  let (chat, other) = (ChatId::generate(), ChatId::generate());
  let (task, module) = (TaskId::generate(), ModuleId::generate());
  let all = set([
    watch(chat, WatchTarget::Project, 1),
    watch(chat, WatchTarget::Module(module), 2),
    watch(other, WatchTarget::Task(task), 3),
  ]);
  assert_eq!(
    event_subscription(&all, chat, task, Some(module)).map(Subscription::target),
    Some(WatchTarget::Module(module)),
    "another conversation's subscription is not this one's"
  );
  assert_eq!(
    event_subscription(&all, chat, task, None).map(Subscription::target),
    Some(WatchTarget::Project)
  );
  assert_eq!(event_subscription(&all, ChatId::generate(), task, None), None);
}
