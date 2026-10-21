//! The Orchestrator merges a finished run on its own behind the person's switch (ORC-69, `docs/design/
//! orchestrator-toolset.md`, decision 5): the chat's rule (`orchestrator::auto_merge`) merges only a task's latest run
//! that succeeded with every step passed (a step automation accepted does not count), of a task not marked Blocked,
//! not blocked by unfinished tasks, not held and not saying *Don't merge automatically*; the follower keeps whether the
//! turn of a run's success merges it or why it is left to the person, only with *merge finished runs*, *merge runs*
//! and *answer agents* on; and such turns start one at a time in the order the runs ended, across conversations.
//! `cargo test -p orchester-workspace --test orchestrator_auto_merge auto_merge`.

mod support;

use std::collections::HashSet;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ConnectionId, TaskId},
  name::Name,
  orchestrator::{
    ActionOutcome, AgentWait, Chat, ChatAction, EntryKind, MergeLeft, Merging, OrchestratorSettings, RunEnd,
    TurnState, WaitKind, WaitTurn,
  },
  resource::ModelId,
  revision::Revision,
  run::{Answerer, Run, StepFailure, StuckDecision},
  store::ConnectionStore,
  task::organization::{BlockingTask, ChangeActor, DependencyGate, TaskOrganization},
  task_group::{HoldActor, HoldScope},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::orchestrator::{
  auto_merge::merging_in,
  waits::{Followed, Switches, check_merges_in, follow_in},
};
use support::{Fixture, at};

/// Every switch a turn that merges on its own needs: *answer agents*, and *merge finished runs* with *merge runs*.
const MERGING: Switches = Switches {
  answers: true,
  continues: false,
  merges: true,
};

/// A checked, signed-in Claude Code connection, published as what the Orchestrator runs on.
fn ready(fixture: &mut Fixture) {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude · Kontur").expect("name"),
    AgentProvider::ClaudeCodeCli,
    fixture.repo.path().join("claude.exe"),
    Timestamp::now(),
  )
  .expect("absolute program");
  let haiku = CatalogModel::new(ModelId::new("haiku").expect("model"), Vec::new());
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "2.1.89".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, vec![haiku]),
      },
    })
    .expect("check");
  fixture.db.create_connection(&connection).expect("connection");
  let settings = OrchestratorSettings::new(connection.id(), ModelId::new("haiku").expect("model"), None);
  fixture
    .db
    .publish_orchestrator(&settings, Timestamp::now())
    .expect("published");
}

/// A task titled `title` and its run, which succeeded now.
fn succeeded(fixture: &mut Fixture, title: &str) -> (TaskId, Run) {
  let task = fixture.task(title, at(40));
  let mut run = fixture.started_run(task, at(50));
  fixture.succeed(&mut run, Timestamp::now());
  (task, run)
}

/// Follows the ends with the project's runner attached, under `switches`.
fn follow(fixture: &Fixture, switches: Switches) -> Followed {
  let (root, programs) = (fixture.repo.path(), Programs::processes());
  let checks = check_merges_in((root, &programs), &HashSet::new()).expect("checked");
  follow_in((root, fixture.db.project().id(), &programs), &checks, (switches, true)).expect("followed")
}

/// The waits of conversation `chat`, with their turns.
fn waits_in(fixture: &Fixture, chat: ChatId) -> Vec<(AgentWait, Option<WaitTurn>)> {
  fixture
    .db
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Wait { wait, turn } => Some((wait, turn)),
      _ => None,
    })
    .collect()
}

/// The waits of the project's latest conversation.
fn waits(fixture: &Fixture) -> Vec<(AgentWait, Option<WaitTurn>)> {
  match fixture.db.latest_chat().expect("read") {
    Some(chat) => waits_in(fixture, chat),
    None => Vec::new(),
  }
}

/// What the rule says of `run` now.
fn rule(fixture: &Fixture, run: &Run) -> Merging {
  merging_in(&ProjectDb::open(fixture.repo.path()).expect("database"), run.id()).expect("read")
}

fn left(why: MergeLeft) -> Merging {
  Merging::Left { why }
}

/// `task`'s organization changed as `change` says, by the person.
fn organize(fixture: &mut Fixture, task: TaskId, change: impl FnOnce(TaskOrganization) -> TaskOrganization) {
  fixture
    .db
    .update_task_organization(task, ChangeActor::Person, Timestamp::now(), change)
    .expect("organized");
}

#[test]
fn auto_merge_merges_a_latest_run_that_succeeded_and_whose_task_nothing_holds() {
  let mut fixture = Fixture::new();
  let (_, run) = succeeded(&mut fixture, "Add login");
  assert_eq!(rule(&fixture, &run), Merging::Merges);
  // A later run of the task makes this one not its latest.
  let task = run.task();
  fixture.started_run(task, Timestamp::now());
  assert_eq!(rule(&fixture, &run), left(MergeLeft::NotReady));
}

#[test]
fn auto_merge_leaves_an_opted_out_blocked_or_held_task_to_the_person() {
  let mut fixture = Fixture::new();
  let (task, run) = succeeded(&mut fixture, "Add login");
  organize(&mut fixture, task, |value| value.with_no_automatic_merge(true));
  assert_eq!(rule(&fixture, &run), left(MergeLeft::OptedOut));
  organize(&mut fixture, task, |value| value.with_no_automatic_merge(false));
  assert_eq!(rule(&fixture, &run), Merging::Merges, "the flag cleared, it merges again");

  let marked = |value: TaskOrganization| {
    TaskOrganization::new(
      value.status(),
      value.priority(),
      Some("Waits for the release".to_owned()),
      value.dependencies().to_vec(),
      value.tags().to_vec(),
    )
    .expect("marked Blocked")
  };
  organize(&mut fixture, task, marked);
  assert_eq!(
    rule(&fixture, &run),
    left(MergeLeft::MarkedBlocked {
      reason: "Waits for the release".to_owned()
    })
  );

  let mut fixture = Fixture::new();
  let blocker = fixture.task("Ship the API", at(30));
  let (task, run) = succeeded(&mut fixture, "Add login");
  let linked = move |value: TaskOrganization| {
    TaskOrganization::new(
      value.status(),
      value.priority(),
      None,
      vec![BlockingTask {
        task: blocker,
        gate: DependencyGate::Merge,
      }],
      value.tags().to_vec(),
    )
    .expect("linked")
  };
  organize(&mut fixture, task, linked);
  assert_eq!(
    rule(&fixture, &run),
    left(MergeLeft::BlockedByTasks {
      tasks: vec!["ORC-1".to_owned()]
    }),
    "the unfinished blocking task, by its key"
  );

  let mut fixture = Fixture::new();
  let (_, run) = succeeded(&mut fixture, "Add login");
  fixture
    .db
    .set_queue_hold((HoldScope::Project, HoldActor::Person), true, Timestamp::now())
    .expect("held");
  assert_eq!(rule(&fixture, &run), left(MergeLeft::Held));
}

/// The run of a task whose one step waited unverified and was accepted `by`, then passed.
fn accepted(fixture: &mut Fixture, by: Answerer) -> Run {
  let task = fixture.task("Check the import", at(40));
  let mut run = fixture.started_run(task, at(50));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let revision = Revision::new(&"b".repeat(40)).expect("revision");
  let events = run
    .fail_judged(execution, (StepFailure::Unverified, Some(revision)), Timestamp::now())
    .expect("unverified");
  fixture.record(&run, &events);
  let events = run
    .decide_stuck(&StuckDecision::Accept { by }, Timestamp::now())
    .expect("accepted");
  fixture.record(&run, &events);
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.start_step(execution, Timestamp::now()).expect("started");
  fixture.record(&run, &events);
  fixture.succeed(&mut run, Timestamp::now());
  run
}

#[test]
fn auto_merge_counts_a_step_a_person_accepted_and_not_one_automation_accepted() {
  let mut fixture = Fixture::new();
  let person = accepted(&mut fixture, Answerer::Person);
  assert_eq!(rule(&fixture, &person), Merging::Merges);
  let mut fixture = Fixture::new();
  let automation = accepted(&mut fixture, Answerer::Automation);
  assert_eq!(
    rule(&fixture, &automation),
    left(MergeLeft::AcceptedByAutomation {
      step: "Implement".to_owned()
    })
  );
}

#[test]
fn auto_merge_marks_a_success_only_with_every_switch_on() {
  for (switches, said) in [
    (
      Switches {
        merges: false,
        ..MERGING
      },
      "merge finished runs or merge runs off",
    ),
    (
      Switches {
        answers: false,
        ..MERGING
      },
      "answer agents off: no turn at all",
    ),
  ] {
    let mut fixture = Fixture::new();
    ready(&mut fixture);
    succeeded(&mut fixture, "Add login");
    follow(&fixture, switches);
    assert_eq!(waits(&fixture)[0].0.merging, None, "{said}");
  }
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let (_, run) = succeeded(&mut fixture, "Add login");
  let followed = follow(&fixture, MERGING);
  let (wait, turn) = &waits(&fixture)[0];
  assert_eq!(wait.kind, WaitKind::Ended(RunEnd::Succeeded));
  assert_eq!(wait.merging, Some(Merging::Merges), "{wait:?}");
  assert_eq!(turn.as_ref().map(|turn| turn.message), followed.turn.map(|turn| turn.message));
  let prompt = fixture
    .db
    .chat_turn(followed.turn.expect("a turn").message)
    .expect("read")
    .expect("the turn")
    .text;
  assert!(
    prompt
      .as_str()
      .contains(&format!("merge this run once with merge_run (run #{})", run.number().get())),
    "{}",
    prompt.as_str()
  );
}

#[test]
fn auto_merge_of_an_opted_out_task_only_tells_the_person() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let (task, _) = succeeded(&mut fixture, "Add login");
  organize(&mut fixture, task, |value| value.with_no_automatic_merge(true));
  let followed = follow(&fixture, MERGING);
  assert_eq!(waits(&fixture)[0].0.merging, Some(left(MergeLeft::OptedOut)));
  let prompt = fixture
    .db
    .chat_turn(followed.turn.expect("the turn still tells the person").message)
    .expect("read")
    .expect("the turn")
    .text;
  assert!(prompt.as_str().contains("Do not merge it"), "{}", prompt.as_str());
}

/// A conversation, started at `at(start)`, whose answered turn updated task `key`: the one that dealt with it.
fn dealt_with(fixture: &mut Fixture, key: &str, start: i64) -> Chat {
  let chat = Chat::named(ChatId::generate(), key, at(start));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Look after it").expect("text");
  fixture
    .db
    .keep_chat_message(&chat, (person, &text), at(start))
    .expect("kept");
  let action = ChatAction::new("update_task", Some(key.to_owned()), ActionOutcome::Done);
  fixture
    .db
    .add_chat_action(chat.id(), &action, at(start + 1))
    .expect("kept");
  fixture
    .db
    .end_chat_turn(person, &TurnState::Answered, at(start + 2))
    .expect("ended");
  chat
}

/// The turn of the one wait of `chat`.
fn turn_in(fixture: &Fixture, chat: &Chat) -> Option<WaitTurn> {
  waits_in(fixture, chat.id())[0].1.clone()
}

#[test]
fn auto_merge_turns_start_one_at_a_time_in_the_order_the_runs_ended() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let first_task = fixture.task("Add login", at(40));
  let second_task = fixture.task("Add logout", at(41));
  let first_chat = dealt_with(&mut fixture, "ORC-1", 10);
  let second_chat = dealt_with(&mut fixture, "ORC-2", 20);
  let mut first = fixture.started_run(first_task, at(50));
  let mut second = fixture.started_run(second_task, at(51));
  fixture.succeed(&mut first, Timestamp::now());
  fixture.succeed(&mut second, Timestamp::now());
  let started = follow(&fixture, MERGING).turn.expect("the first end's turn");
  assert_eq!(started.chat, first_chat.id(), "the run that ended first merges first");
  assert_eq!(follow(&fixture, MERGING).turn, None, "the second waits, though its conversation is free");
  assert_eq!(turn_in(&fixture, &second_chat), None);
  fixture
    .db
    .end_chat_turn(started.message, &TurnState::Answered, Timestamp::now())
    .expect("the first turn ended");
  let next = follow(&fixture, MERGING).turn.expect("the second end's turn");
  assert_eq!(next.chat, second_chat.id());
  assert_eq!(waits_in(&fixture, second_chat.id())[0].0.run, second.id());
}

#[test]
fn auto_merge_holds_back_no_turn_that_does_not_merge() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let first_task = fixture.task("Add login", at(40));
  let second_task = fixture.task("Add logout", at(41));
  dealt_with(&mut fixture, "ORC-1", 10);
  let second_chat = dealt_with(&mut fixture, "ORC-2", 20);
  let mut first = fixture.started_run(first_task, at(50));
  let mut second = fixture.started_run(second_task, at(51));
  fixture.succeed(&mut first, Timestamp::now());
  fixture.succeed(&mut second, Timestamp::now());
  let answering = Switches {
    merges: false,
    ..MERGING
  };
  follow(&fixture, answering).turn.expect("the first end's turn");
  let next = follow(&fixture, answering).turn.expect("the second starts beside it");
  assert_eq!(next.chat, second_chat.id());
  // An end that the rule leaves to the person does not wait for a merge either.
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let first_task = fixture.task("Add login", at(40));
  let second_task = fixture.task("Add logout", at(41));
  dealt_with(&mut fixture, "ORC-1", 10);
  let second_chat = dealt_with(&mut fixture, "ORC-2", 20);
  let mut first = fixture.started_run(first_task, at(50));
  let mut second = fixture.started_run(second_task, at(51));
  organize(&mut fixture, second_task, |value| value.with_no_automatic_merge(true));
  fixture.succeed(&mut first, Timestamp::now());
  fixture.succeed(&mut second, Timestamp::now());
  follow(&fixture, MERGING).turn.expect("the first end's turn, which merges");
  let told = follow(&fixture, MERGING).turn.expect("the opted-out end only tells the person");
  assert_eq!(told.chat, second_chat.id());
  assert_eq!(waits_in(&fixture, second_chat.id())[0].0.run, second.id());
}
