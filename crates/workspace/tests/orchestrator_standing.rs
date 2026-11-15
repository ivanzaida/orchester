//! Where what a conversation watches stands, read from a project (`orchestrator::standing`, OA-12), and the events a
//! subscription that leads to the end raises from it, once per state (OA-13, OA-15): work can start, the target is
//! stuck once what is left is parked, and work can start again once the person gave the task back.
//! `cargo test -p orchester-workspace --test orchestrator_standing standing`.

mod support;

use std::collections::HashSet;

use orchester_core::{
  id::{ChatId, IdempotencyKey, TaskId},
  name::Name,
  orchestrator::{
    Ceilings, Chat, EntryKind, Subscription, WatchMode, WatchTarget,
    parking::{Needs, Parking, Words},
    standing::{Raised, StandingEvent, TargetStanding},
  },
  store::TaskStore,
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  orchestrator::{
    parking::give_back_in,
    standing::Reads,
    waits::{Switches, check_merges_in, follow_in},
  },
  tasks::data::published_pipelines,
};
use support::{Fixture, at};

/// A task that can start: a draft on the fixture's published pipeline.
fn draft(fixture: &mut Fixture, title: &str, now: Timestamp) -> TaskId {
  let pipeline = published_pipelines(&fixture.db).expect("pipelines")[0].id;
  let details = TaskDetails {
    title: Name::new(title).expect("title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: Some(pipeline),
    limits: TaskLimits::NONE,
  };
  fixture
    .db
    .create_tasks(IdempotencyKey::generate(), vec![details], now)
    .expect("task")[0]
    .id()
}

fn follow(fixture: &Fixture) {
  let (root, programs) = (fixture.repo.path(), Programs::processes());
  let checks = check_merges_in((root, &programs), &HashSet::new()).expect("checked");
  let project = fixture.db.project().id();
  follow_in((root, project, &programs), &checks, (Switches { drives: true }, true)).expect("followed");
}

/// The events of led targets `chat` keeps, oldest first.
fn events(fixture: &Fixture, chat: ChatId) -> Vec<StandingEvent> {
  let entries = fixture.db.chat_entries(chat).expect("entries");
  entries
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Standing { event, .. } => Some(*event),
      _ => None,
    })
    .collect()
}

#[test]
fn standing_reads_each_task_of_what_a_subscription_watches() {
  let mut fixture = Fixture::new();
  let chat = fixture.everything(at(100));
  draft(&mut fixture, "Add login", at(110));
  fixture.task("No pipeline yet", at(120));
  let working = fixture.task("Billing", at(130));
  fixture.started_run(working, at(140));
  let finished = fixture.task("Ship the docs", at(150));
  let mut run = fixture.started_run(finished, at(160));
  fixture.succeed(&mut run, at(170));

  let all = Reads::read(&fixture.db, fixture.repo.path())
    .expect("read")
    .all(&fixture.db)
    .expect("standing");
  assert_eq!(all.len(), 1, "one subscription: {all:?}");
  let led = &all[0];
  assert_eq!(led.subscription.chat(), chat.id());
  assert_eq!(led.name, None, "the whole project has no name of its own");
  let named: Vec<(&str, &str)> = led
    .standing
    .tasks
    .iter()
    .map(|(task, standing)| (task.as_str(), standing.name()))
    .collect();
  assert_eq!(
    named,
    vec![
      ("ORC-1", "ready_to_start"),
      ("ORC-2", "no_pipeline"),
      ("ORC-3", "working"),
      ("ORC-4", "to_merge"),
    ]
  );
  assert_eq!(
    led.standing.target,
    TargetStanding::CanStart {
      tasks: vec!["ORC-1".to_owned()],
      slot_free: true
    }
  );
  assert!(led.parked.is_empty());
}

#[test]
fn standing_raises_work_that_can_start_and_a_stuck_target_once_per_state() {
  let mut fixture = Fixture::new();
  let chat: Chat = fixture.everything(at(100));
  let task = draft(&mut fixture, "Add login", at(110));

  // Only followed: its standing raises nothing.
  follow(&fixture);
  assert!(
    events(&fixture, chat.id()).is_empty(),
    "a subscription that follows raises no event"
  );

  let lead = WatchMode::Lead {
    ceilings: Ceilings::default(),
  };
  let leading = Subscription::new(chat.id(), WatchTarget::Project, "", at(200)).in_mode(lead);
  // The person stops following and has it led to the end instead.
  fixture
    .db
    .stop_subscription(chat.id(), WatchTarget::Project, at(190))
    .expect("stopped");
  fixture.db.start_subscription(&leading).expect("leads to the end");
  follow(&fixture);
  follow(&fixture);
  let raised = events(&fixture, chat.id());
  assert_eq!(raised.len(), 1, "once for the state it is in: {raised:?}");
  assert_eq!(
    raised[0].raised,
    Raised::CanStart {
      tasks: vec!["ORC-1".to_owned()]
    }
  );
  assert!(
    raised[0].automatic,
    "kept while the chat drives: it starts a turn by itself"
  );

  // The chat parks the task for the person: nothing can start and nothing moves.
  let parking = Parking {
    task,
    run: None,
    needs: Needs::Decision,
    why: Words::new("The brief does not say which provider to use.").expect("words"),
    to_do: Words::new("Name the provider in the task's brief.").expect("words"),
    by: chat.id(),
    at: at(300),
  };
  fixture.db.park_task(&parking).expect("parked");
  follow(&fixture);
  follow(&fixture);
  let raised = events(&fixture, chat.id());
  assert_eq!(raised.len(), 2, "{raised:?}");
  let Raised::Stuck { left, budget_used } = &raised[1].raised else {
    panic!("the target is stuck: {:?}", raised[1]);
  };
  assert!(!budget_used);
  assert_eq!(left.len(), 1, "{left:?}");
  assert_eq!((left[0].task.as_str(), left[0].standing.as_str()), ("ORC-1", "parked"));
  assert_eq!(
    left[0].note.as_deref(),
    Some("Name the provider in the task's brief."),
    "what the person is to do"
  );

  // The person gives it back: the state it was in before is raised again, since it changed in between.
  assert!(give_back_in(fixture.repo.path(), task).expect("given back"));
  follow(&fixture);
  let raised = events(&fixture, chat.id());
  assert_eq!(raised.len(), 3, "{raised:?}");
  assert!(matches!(raised[2].raised, Raised::CanStart { .. }));
}
