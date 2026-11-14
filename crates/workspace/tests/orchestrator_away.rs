//! OA-23: what a person who comes back reads first (`orchestrator::away`): the tasks parked for them, oldest first
//! and named as people read them, and the Orchestrator's decisions they have not looked at, newest first with the
//! module of each one's task; marking a decision seen or overruled takes it off the list, and a task given back is
//! no longer parked.
//! `cargo test -p orchester-workspace --test orchestrator_away away`.

mod support;

use orchester_core::{
  id::{IdempotencyKey, TaskId},
  module::{ModuleDetails, ModuleState},
  name::Name,
  orchestrator::{
    Chat,
    decision::{NewDecision, Reason},
    parking::{Needs, Parking, Words},
  },
  store::TaskStore,
  task::{TaskDetails, TaskLimits, organization::ChangeActor},
  time::Timestamp,
};
use orchester_workspace::{
  orchestrator::{
    away::{away_in, mark_seen_in, overrule_in},
    parking::give_back_in,
  },
  tasks::data::published_pipelines,
};
use support::{Fixture, at};

fn task(fixture: &mut Fixture, title: &str, now: Timestamp) -> TaskId {
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

fn decided(chat: &Chat, tool: &str, task: Option<&str>, when: i64) -> NewDecision {
  NewDecision {
    chat: chat.id(),
    tool: tool.to_owned(),
    task: task.map(str::to_owned),
    run: None,
    reason: Reason::new("Every step passed and the order allows it.").expect("a reason"),
    undo: None,
    at: at(when),
  }
}

#[test]
fn away_reads_what_is_parked_and_what_was_decided_and_forgets_what_the_person_settled() {
  let mut fixture = Fixture::new();
  let chat: Chat = fixture.everything(at(100));
  let first = task(&mut fixture, "Publish the webhook schema", at(110));
  let second = task(&mut fixture, "Retry failed deliveries", at(120));
  let details = ModuleDetails::new(
    Name::new("Webhooks v2").expect("name"),
    "",
    (None, None),
    ModuleState::Open,
  )
  .expect("details");
  let module = fixture.db.create_module(details, at(130)).expect("module").id();
  fixture
    .db
    .set_task_module(first, Some(module), ChangeActor::Person, at(131))
    .expect("in the module");
  let root = fixture.repo.path().to_path_buf();
  assert!(away_in(&root).expect("read").is_empty(), "nothing yet");

  let parking = |task, when| Parking {
    task,
    run: None,
    needs: Needs::Approval,
    why: Words::new("The Orchestrator may not allow a push outside the project.").expect("words"),
    to_do: Words::new("Allow or deny it on the request's card.").expect("words"),
    by: chat.id(),
    at: at(when),
  };
  fixture.db.park_task(&parking(second, 300)).expect("parked");
  fixture.db.park_task(&parking(first, 200)).expect("parked");
  let merged = fixture
    .db
    .keep_decision(&decided(&chat, "continue_run", Some("ORC-1"), 400))
    .expect("kept");
  let ran = fixture
    .db
    .keep_decision(&decided(&chat, "run_task", Some("orc-2"), 500))
    .expect("kept");
  let other = fixture
    .db
    .keep_decision(&decided(&chat, "deliver_module", None, 450))
    .expect("kept");

  let away = away_in(&root).expect("read");
  let parked: Vec<(&str, &str)> = away
    .parked
    .iter()
    .map(|parked| (parked.key.as_str(), parked.title.as_str()))
    .collect();
  assert_eq!(
    parked,
    [
      ("ORC-1", "Publish the webhook schema"),
      ("ORC-2", "Retry failed deliveries")
    ],
    "oldest first"
  );
  let decisions: Vec<(i64, Option<&str>)> = away
    .decisions
    .iter()
    .map(|open| (open.decision.number, open.module.as_deref()))
    .collect();
  assert_eq!(
    decisions,
    [(ran, None), (other, None), (merged, Some("Webhooks v2"))],
    "newest first, each with its task's module"
  );

  // Seen and overruled decisions are no longer open; a task given back is no longer parked.
  mark_seen_in(&root, &[ran, other], at(600)).expect("seen");
  overrule_in(&root, merged, at(610)).expect("overruled");
  assert!(give_back_in(&root, first).expect("given back"));
  let away = away_in(&root).expect("read");
  assert!(away.decisions.is_empty(), "{:?}", away.decisions);
  assert_eq!(away.parked.len(), 1);
  assert_eq!(away.parked[0].key, "ORC-2");
}
