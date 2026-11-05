//! Which Orchestrator conversation leads a task's events (`orchestrator::watch`): the most specific subscription,
//! one leader per target, the task inside a module another conversation leads, taking a target over and stopping.

use orchester_core::{
  id::{ChatId, ModuleId, TaskId},
  orchestrator::{
    Claim, Subscription, Subscriptions, WatchTarget,
    watch::{News, TwoLeaders},
  },
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
