//! The Orchestrator chat follows the ends of its project's runs (`orchestrator::waits`, `docs/design/
//! orchestrator-toolset.md`, decision 5): a task's latest run that succeeded, failed, stopped for a decision, stopped
//! at a limit or was interrupted is kept once, linking to the task's Summary, in the conversation that last dealt with
//! its task, else the latest one; the runs the daily budget stopped at once are one entry; ends from before the project
//! followed them, a pause, *Quit now* or a stop the person asked for, an interruption the runner continues by itself
//! and a run merged are not kept; with *Let the Orchestrator answer agents* on an end starts one Orchestrator turn by
//! itself, with the prompt its end calls for, but none for an end the chat's own decision on or start of the run led
//! to, and none while the switch is off.
//! `cargo test -p orchester-workspace --test orchestrator_run_ends run_ends`.

mod support;

use std::collections::HashSet;

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, ConnectionId, ProjectId},
  integration::{MergeKind, RunMerge},
  name::Name,
  orchestrator::{
    ActionOutcome, AgentWait, Chat, ChatAction, Continuing, EntryKind, OrchestratorSettings, RunEnd, StopReason,
    TurnState, WaitKind, WaitTurn,
    change::{Change, ContinueChoice, ProposedChange, StepName},
  },
  resource::ModelId,
  revision::Revision,
  run::{BranchName, InterruptCause, Run, RunLimit, RunnerFailure, StepFailure, StuckDecision},
  store::{ConnectionStore, MergeStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::{
  orchestrator::waits::{Followed, MergeChecks, Switches, check_merges_in, follow_in},
  tasks::changes::run_worktree,
};
use support::{Fixture, at};

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

/// A run of task `title` at work.
fn working(fixture: &mut Fixture, title: &str) -> Run {
  let task = fixture.task(title, at(40));
  fixture.started_run(task, at(50))
}

/// `run`'s one step fails at `now` for `failure`.
fn fail(fixture: &mut Fixture, run: &mut Run, failure: StepFailure) {
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let events = run.fail_step(execution, failure, Timestamp::now()).expect("failed");
  fixture.record(run, &events);
}

fn project(fixture: &Fixture) -> ProjectId {
  fixture.db.project().id()
}

/// Follows the ends with the project's runner attached, *Let the Orchestrator answer agents* on when `answers`.
fn follow(fixture: &Fixture, answers: bool) -> Followed {
  follow_with(
    fixture,
    Switches {
      answers,
      continues: false,
    },
  )
}

/// Follows the ends with the project's runner attached, under `switches`.
fn follow_with(fixture: &Fixture, switches: Switches) -> Followed {
  let (root, programs) = (fixture.repo.path(), Programs::processes());
  let checks = check_merges_in((root, &programs), &HashSet::new()).expect("checked");
  follow_in((root, project(fixture), &programs), &checks, (switches, true)).expect("followed")
}

/// The waits of conversation `chat`, with their turns.
fn waits_in(fixture: &Fixture, chat: ChatId) -> Vec<(AgentWait, Option<WaitTurn>)> {
  let db = ProjectDb::open(fixture.repo.path()).expect("database");
  db.chat_entries(chat)
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

/// A conversation, started at `at(start)`, whose answered turn took `action`.
fn acted(fixture: &mut Fixture, action: &ChatAction, start: i64) -> Chat {
  let chat = Chat::named(ChatId::generate(), "Look after the runs", at(start));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Look after the runs").expect("text");
  fixture
    .db
    .keep_chat_message(&chat, (person, &text), at(start))
    .expect("kept");
  fixture
    .db
    .add_chat_action(chat.id(), action, at(start + 1))
    .expect("kept");
  fixture
    .db
    .end_chat_turn(person, &TurnState::Answered, at(start + 2))
    .expect("ended");
  chat
}

#[test]
fn run_ends_are_kept_once_in_the_conversation_that_dealt_with_their_task() {
  let mut fixture = Fixture::new();
  let mut done = working(&mut fixture, "Add login");
  let mut broken = working(&mut fixture, "Add logout");
  let dealt = acted(
    &mut fixture,
    &ChatAction::new("update_task", Some("ORC-1".to_owned()), ActionOutcome::Done),
    10,
  );
  let latest = acted(
    &mut fixture,
    &ChatAction::new("create_task", Some("ORC-9".to_owned()), ActionOutcome::Done),
    20,
  );
  fixture.succeed(&mut done, Timestamp::now());
  fail(&mut fixture, &mut broken, StepFailure::ReportedFailed);
  assert!(follow(&fixture, false).kept);
  let ready = waits_in(&fixture, dealt.id());
  assert_eq!(ready.len(), 1, "{ready:?}");
  let (wait, turn) = &ready[0];
  assert_eq!(wait.kind, WaitKind::Ended(RunEnd::Succeeded));
  assert_eq!((wait.task.as_str(), wait.number), ("ORC-1", 1));
  assert_eq!(wait.link, format!("/project/{}/tasks/ORC-1/summary", project(&fixture)));
  assert!(!wait.automatic && turn.is_none(), "the switch is off: {wait:?}");
  let failed = waits_in(&fixture, latest.id());
  assert_eq!(failed.len(), 1, "{failed:?}");
  assert_eq!(failed[0].0.kind, WaitKind::Ended(RunEnd::Failed));
  assert_eq!(failed[0].0.step.as_deref(), Some("Implement"), "the step it failed at");
  assert_eq!(follow(&fixture, false), Followed::default(), "each end is kept once");
}

#[test]
fn run_ends_name_how_the_run_stopped_and_leave_out_what_is_no_end() {
  let mut fixture = Fixture::new();
  let mut old = working(&mut fixture, "Old work");
  fixture.succeed(&mut old, at(60));
  let mut paused = working(&mut fixture, "Paused by the person");
  let events = paused.pause(Timestamp::now()).expect("pause asked");
  fixture.record(&paused, &events);
  for (title, cause) in [
    ("Runner lost", InterruptCause::RunnerLost),
    ("Quit now", InterruptCause::QuitNow),
  ] {
    let mut run = working(&mut fixture, title);
    let events = run.interrupt(cause, Timestamp::now()).expect("interrupted");
    fixture.record(&run, &events);
  }
  assert_eq!(
    follow(&fixture, false),
    Followed::default(),
    "history, a person's pause or Quit now and a run the runner continues itself are no ends"
  );
  let mut limited = working(&mut fixture, "Spent its budget");
  let events = limited
    .stop_at_limit(RunLimit::Tokens, Timestamp::now())
    .expect("stopped");
  fixture.record(&limited, &events);
  let mut crashed = working(&mut fixture, "Crashed");
  let events = crashed
    .interrupt(InterruptCause::Crashed, Timestamp::now())
    .expect("interrupted");
  fixture.record(&crashed, &events);
  let mut unverified = working(&mut fixture, "Could not check");
  fail(&mut fixture, &mut unverified, StepFailure::Unverified);
  assert!(follow(&fixture, false).kept);
  let mut kept: Vec<(String, WaitKind)> = waits(&fixture)
    .into_iter()
    .map(|(wait, _)| (wait.title, wait.kind))
    .collect();
  kept.sort_by(|a, b| a.0.cmp(&b.0));
  assert_eq!(
    kept,
    vec![
      (
        "Could not check".to_owned(),
        WaitKind::Ended(RunEnd::Stopped {
          why: StopReason::Unverified
        })
      ),
      ("Crashed".to_owned(), WaitKind::Ended(RunEnd::Interrupted)),
      (
        "Spent its budget".to_owned(),
        WaitKind::Ended(RunEnd::Limit {
          limit: RunLimit::Tokens
        })
      ),
    ]
  );
}

#[test]
fn run_ends_start_one_turn_with_their_prompt_while_answering_agents_is_on() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let mut run = working(&mut fixture, "Add login");
  fixture.succeed(&mut run, Timestamp::now());
  let started = follow(&fixture, true).turn.expect("the end starts a turn by itself");
  assert_eq!(follow(&fixture, true).turn, None, "one turn per end");
  let (wait, turn) = &waits(&fixture)[0];
  assert!(wait.automatic && !wait.own, "{wait:?}");
  assert_eq!(turn.as_ref().map(|turn| turn.message), Some(started.message));
  let prompt = fixture.db.chat_turn(started.message).expect("read").expect("the turn");
  for part in [
    "Run #1 of ORC-1 (Add login) succeeded: its work is ready to merge",
    "If the person asked you earlier in this conversation to merge this task's run",
  ] {
    assert!(
      prompt.text.as_str().contains(part),
      "{part} in {}",
      prompt.text.as_str()
    );
  }
  fixture
    .db
    .end_chat_turn(started.message, &TurnState::Answered, Timestamp::now())
    .expect("ended");
  assert_eq!(follow(&fixture, true), Followed::default(), "the end stays one");
}

#[test]
fn run_ends_the_chats_own_decision_led_to_start_no_turn() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let mut run = working(&mut fixture, "Add login");
  let retried = ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: "Add login".to_owned(),
      change: Change::Continue {
        run: run.id(),
        number: 1,
        attempt: 1,
        step: Some(StepName {
          place: 1,
          name: "Implement".to_owned(),
        }),
        why: None,
        choice: ContinueChoice::AllowRetry,
      },
    })),
    ..ChatAction::new("continue_run", Some("ORC-1".to_owned()), ActionOutcome::Applied)
  };
  let chat = acted(&mut fixture, &retried, 10);
  fail(&mut fixture, &mut run, StepFailure::ReportedFailed);
  let followed = follow(&fixture, true);
  assert!(followed.kept && followed.turn.is_none(), "{followed:?}");
  let (wait, turn) = &waits_in(&fixture, chat.id())[0];
  assert!(wait.own && !wait.automatic && turn.is_none(), "{wait:?}");
}

#[test]
fn run_end_turns_name_what_the_person_chooses_for_a_run_that_stopped() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let mut run = working(&mut fixture, "Add login");
  fail(&mut fixture, &mut run, StepFailure::Unverified);
  let started = follow(&fixture, true).turn.expect("a turn");
  let prompt = fixture.db.chat_turn(started.message).expect("read").expect("the turn");
  for part in [
    "Run #1 of ORC-1 (Add login) stopped at the step Implement (its step could not check everything) and waits",
    "continue_run",
    "the person applies",
  ] {
    assert!(
      prompt.text.as_str().contains(part),
      "{part} in {}",
      prompt.text.as_str()
    );
  }
}

#[test]
fn run_ends_leave_out_a_persons_stop_and_a_merged_run() {
  let mut fixture = Fixture::new();
  let mut stopped = working(&mut fixture, "Could not check");
  fail(&mut fixture, &mut stopped, StepFailure::Unverified);
  assert!(follow(&fixture, false).kept, "it waits for a decision");
  let events = stopped
    .decide_stuck(&StuckDecision::Stop, Timestamp::now())
    .expect("stopped by the person");
  fixture.record(&stopped, &events);
  let mut merged = working(&mut fixture, "Merged at once");
  fixture.succeed(&mut merged, Timestamp::now());
  fixture
    .db
    .save_run_merge(&RunMerge {
      run: merged.id(),
      into: BranchName::new("main").expect("branch"),
      revision: Revision::new(&"a".repeat(40)).expect("revision"),
      kind: MergeKind::FastForward,
      merged_at: Timestamp::now(),
    })
    .expect("merged");
  assert_eq!(
    follow(&fixture, false),
    Followed::default(),
    "the person's stop and a run merged before it was noticed are no ends"
  );
  assert_eq!(waits(&fixture).len(), 1, "only the wait for the decision");
}

#[test]
fn run_ends_keep_the_runs_the_daily_budget_stopped_at_once_as_one() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  for title in ["First", "Second", "Third"] {
    let mut run = working(&mut fixture, title);
    let events = run
      .stop_at_limit(RunLimit::DailyBudget, Timestamp::now())
      .expect("stopped");
    fixture.record(&run, &events);
  }
  let followed = follow(&fixture, true);
  assert!(
    followed.kept && followed.turn.is_some(),
    "one turn for the limit: {followed:?}"
  );
  let kept = waits(&fixture);
  assert_eq!(kept.len(), 1, "{kept:?}");
  assert_eq!(kept[0].0.others, 2);
  assert_eq!(follow(&fixture, true).turn, None, "one turn per limit");
}

#[test]
fn run_ends_of_a_run_the_chat_started_start_no_turn() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let mut run = working(&mut fixture, "Add login");
  let started = ChatAction {
    started: Some(run.id()),
    auto_mode: true,
    ..ChatAction::new("run_task", Some("ORC-1".to_owned()), ActionOutcome::Done)
  };
  let chat = acted(&mut fixture, &started, 10);
  fail(&mut fixture, &mut run, StepFailure::ReportedFailed);
  let followed = follow(&fixture, true);
  assert!(followed.kept && followed.turn.is_none(), "{followed:?}");
  let (wait, _) = &waits_in(&fixture, chat.id())[0];
  assert!(wait.own && !wait.automatic, "{wait:?}");
}

#[test]
fn run_ends_leave_out_a_run_git_tells_merged_outside_or_with_nothing_to_merge() {
  let mut fixture = Fixture::new();
  let mut run = working(&mut fixture, "Nothing to merge");
  fixture.succeed(&mut run, Timestamp::now());
  let checks = MergeChecks {
    quiet: HashSet::from([(run.id(), run.last_sequence().get())]),
    waiting: HashSet::new(),
  };
  let followed = follow_in(
    (fixture.repo.path(), project(&fixture), &Programs::processes()),
    &checks,
    (Switches::default(), true),
  )
  .expect("followed");
  assert_eq!(followed, Followed::default(), "never told as ready to merge");
}

/// Both of the person's switches a continuing turn needs: *answer agents* and *continue interrupted runs*.
const CONTINUING: Switches = Switches {
  answers: true,
  continues: true,
};

/// `run` interrupted for `cause` now.
fn interrupt(fixture: &mut Fixture, run: &mut Run, cause: InterruptCause) {
  let events = run.interrupt(cause, Timestamp::now()).expect("interrupted");
  fixture.record(run, &events);
}

/// A run of task `title` that `cause` interrupted, its worktree there.
fn interrupted(fixture: &mut Fixture, title: &str, cause: InterruptCause) -> Run {
  let mut run = working(fixture, title);
  std::fs::create_dir_all(run_worktree(fixture.repo.path(), run.id())).expect("its worktree");
  interrupt(fixture, &mut run, cause);
  run
}

/// The prompt of the turn `followed` started.
fn prompt_of(fixture: &Fixture, followed: &Followed) -> String {
  let turn = followed.turn.expect("a turn");
  let kept = fixture.db.chat_turn(turn.message).expect("read").expect("the turn");
  kept.text.as_str().to_owned()
}

#[test]
fn run_ends_of_a_crash_continue_the_run_once_with_both_switches_on() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  interrupted(&mut fixture, "Add login", InterruptCause::Crashed);
  let followed = follow_with(&fixture, CONTINUING);
  let prompt = prompt_of(&fixture, &followed);
  let (wait, _) = &waits(&fixture)[0];
  assert_eq!(wait.continuing, Some(Continuing::Continues), "{wait:?}");
  for part in [
    "Run #1 of ORC-1 (Add login) was interrupted",
    "Let the Orchestrator continue interrupted runs",
    "continue this run once with resume_run, from its last checkpoint",
  ] {
    assert!(prompt.contains(part), "{part} in {prompt}");
  }
  assert_eq!(
    follow_with(&fixture, CONTINUING).turn,
    None,
    "one turn per interruption"
  );
}

#[test]
fn run_ends_of_a_run_interrupted_again_soon_leave_it_to_the_person() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  let mut run = interrupted(&mut fixture, "Add login", InterruptCause::Crashed);
  let first = follow_with(&fixture, CONTINUING)
    .turn
    .expect("the first interruption's turn");
  fixture
    .db
    .end_chat_turn(first.message, &TurnState::Answered, Timestamp::now())
    .expect("ended");
  // A person continued it on the task's Summary, and it crashed again at once.
  let events = run.continue_run(Timestamp::now()).expect("continued");
  fixture.record(&run, &events);
  interrupt(&mut fixture, &mut run, InterruptCause::Crashed);
  let followed = follow_with(&fixture, CONTINUING);
  let prompt = prompt_of(&fixture, &followed);
  let kept = waits(&fixture);
  assert_eq!(kept.len(), 2, "a new end: {kept:?}");
  assert_eq!(kept[1].0.continuing, Some(Continuing::InterruptedAgain));
  for part in [
    "interrupted again within 10 minutes of the time before",
    "left to the person: do not continue it",
    "Start, resume, cancel, continue or merge nothing",
  ] {
    assert!(prompt.contains(part), "{part} in {prompt}");
  }
}

#[test]
fn run_ends_of_an_interruption_only_tell_the_person_without_the_switch_or_the_chats_right() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  interrupted(&mut fixture, "Add login", InterruptCause::Crashed);
  // Answering agents alone: the turn tells the person, as before the switch.
  let followed = follow(&fixture, true);
  let prompt = prompt_of(&fixture, &followed);
  assert_eq!(waits(&fixture)[0].0.continuing, None);
  assert!(prompt.contains("goes on only when a person continues it"), "{prompt}");
  assert!(!prompt.contains("resume_run"), "{prompt}");

  let mut fixture = Fixture::new();
  ready(&mut fixture);
  interrupted(&mut fixture, "Runner failed", InterruptCause::RunnerFailed);
  let mut gone = working(&mut fixture, "Worktree gone");
  interrupt(&mut fixture, &mut gone, InterruptCause::Crashed);
  // Continuing alone starts no turn; with both on, these two stay the person's.
  let alone = Switches {
    answers: false,
    continues: true,
  };
  assert_eq!(follow_with(&fixture, alone).turn, None);
  assert!(
    waits(&fixture)
      .iter()
      .all(|(wait, _)| !wait.automatic && wait.continuing.is_none())
  );
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  interrupted(&mut fixture, "Runner failed", InterruptCause::RunnerFailed);
  let mut gone = working(&mut fixture, "Worktree gone");
  interrupt(&mut fixture, &mut gone, InterruptCause::Crashed);
  follow_with(&fixture, CONTINUING);
  let kept = waits(&fixture);
  assert_eq!(kept.len(), 2, "{kept:?}");
  assert!(
    kept.iter().all(|(wait, _)| wait.automatic && wait.continuing.is_none()),
    "the runner's failure and a gone worktree are the person's: {kept:?}"
  );
}

/// A run of task `title` the runner stopped now because the project database stayed busy, its worktree there.
fn contended(fixture: &mut Fixture, title: &str) -> Run {
  let mut run = working(fixture, title);
  std::fs::create_dir_all(run_worktree(fixture.repo.path(), run.id())).expect("its worktree");
  let events = run
    .interrupt(InterruptCause::RunnerFailed, Timestamp::now())
    .expect("interrupted");
  fixture
    .db
    .append_runner_failure(&run, &events, RunnerFailure::StorageContention)
    .expect("recorded with why");
  run
}

#[test]
fn run_ends_of_storage_contention_continue_once_until_more_than_three_runs_were_so() {
  let mut fixture = Fixture::new();
  ready(&mut fixture);
  contended(&mut fixture, "Add login");
  let followed = follow_with(&fixture, CONTINUING);
  let prompt = prompt_of(&fixture, &followed);
  assert_eq!(waits(&fixture)[0].0.continuing, Some(Continuing::Continues));
  assert!(prompt.contains("continue this run once with resume_run"), "{prompt}");
  for title in ["Add logout", "Add search", "Add export"] {
    contended(&mut fixture, title);
  }
  follow_with(&fixture, CONTINUING);
  let kept = waits(&fixture);
  assert_eq!(kept.len(), 4, "{kept:?}");
  assert!(
    kept[1..]
      .iter()
      .all(|(wait, _)| wait.automatic && wait.continuing == Some(Continuing::StorageContention)),
    "four runs within ten minutes: the database is under contention: {kept:?}"
  );
}
