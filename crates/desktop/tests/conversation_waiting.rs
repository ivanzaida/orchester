//! The task conversation of a run that waits for the person, as data (ORC-79): what the composer
//! offers and who reads a message — a run waiting for a decision is continued by a message from the
//! step it stopped at, a paused or interrupted one resumes with it, one stopped at its own limit keeps
//! it for the raise and one stopped at a shared budget waits for that budget — and where direct
//! messages queued meanwhile stand.

mod support;

use orchester_core::{
  conversation::{Recipient, Timing},
  id::{IdempotencyKey, StepExecutionId, TaskId},
  resource::ModelId,
  run::{InterruptCause, Run, RunLimit, StepFailure},
  store::ActivityStore,
};
use orchester_desktop::tasks::{
  conversation::{
    Author, Conversation, EntryState, PanelState, Queue,
    composer::{ComposerMode, Queued, Then},
    send_in,
  },
  detail::TaskDetail,
};

use self::support::{Fixture, at};

fn read(fixture: &Fixture, task: TaskId) -> Conversation {
  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("the task is read");
  Conversation::of(&detail, false, None)
}

/// What the composer offers for a message to `recipient`.
fn mode(fixture: &Fixture, task: TaskId, recipient: Recipient) -> ComposerMode {
  let detail = TaskDetail::load_in(fixture.repo.path(), task).expect("the task is read");
  let conversation = Conversation::of(&detail, false, None);
  ComposerMode::of(&conversation, detail.run.as_ref(), recipient)
}

fn first_execution(run: &Run) -> StepExecutionId {
  run.current_attempt().expect("attempt").executions()[0].id()
}

#[test]
fn conversation_a_run_that_waits_for_a_decision_is_continued_by_a_message() {
  let mut fixture = Fixture::reviewed();
  let task = fixture.task("Add login", at(10));
  let mut run = fixture.started_run(task, at(20));
  let events = run
    .fail_step(first_execution(&run), StepFailure::AgentFailed, at(30))
    .expect("broke");
  fixture.record(&run, &events);
  let restarted = first_execution(&run);
  let events = run.start_step(restarted, at(31)).expect("restarted");
  fixture.record(&run, &events);
  let events = run
    .fail_step(restarted, StepFailure::AgentFailed, at(32))
    .expect("broke again");
  fixture.record(&run, &events);
  let conversation = read(&fixture, task);
  assert_eq!(conversation.state, PanelState::Deciding);
  assert_eq!(conversation.recovery_after, Some(run.last_sequence()));
  assert_eq!(conversation.next_agent.as_deref(), Some("Builder"));
  assert_eq!(conversation.recipients.len(), 2, "Reviewer never ran");
  let continued = mode(&fixture, task, Recipient::Orchestrator);
  assert_eq!(continued, ComposerMode::Continue);
  assert_eq!(continued.then(), Some(Then::Deliver));
  assert_eq!(
    continued.default_recipient(&conversation),
    conversation.recipients[1].recipient
  );
}

#[test]
fn conversation_suspended_runs_keep_direct_messages_queued_for_the_right_agent_turn() {
  for state in [PanelState::Paused, PanelState::Stopped, PanelState::Interrupted] {
    let mut fixture = Fixture::reviewed();
    let task = fixture.task("Add login", at(10));
    let mut run = fixture.started_run(task, at(20));
    // An interrupted run whose worktree is gone never continues: this one keeps its folder.
    let worktree = fixture
      .repo
      .path()
      .join(".orchester/worktrees")
      .join(run.id().to_string());
    std::fs::create_dir_all(worktree).expect("the run's worktree");
    fixture
      .db
      .start_invocation(
        run.id(),
        first_execution(&run),
        &ModelId::new("gpt-5.5").expect("model"),
        at(20),
      )
      .expect("invocation");
    let events = match state {
      PanelState::Paused => run.pause(at(21)).expect("paused"),
      PanelState::Stopped => run.stop_at_limit(RunLimit::Tokens, at(21)).expect("stopped"),
      PanelState::Interrupted => run.interrupt(InterruptCause::Crashed, at(21)).expect("interrupted"),
      _ => unreachable!("the test lists only suspended states"),
    };
    fixture.record(&run, &events);
    let recipients = read(&fixture, task).recipients;
    for (recipient, text) in [
      (recipients[1].recipient, "Please check the OAuth dialog."),
      (recipients[2].recipient, "Review this after implementation."),
    ] {
      send_in(
        fixture.repo.path(),
        IdempotencyKey::generate(),
        (task, Some(run.id()), None),
        (recipient, Timing::AfterTurn),
        text,
      )
      .expect("direct message stored");
    }

    let conversation = read(&fixture, task);
    assert_eq!(conversation.state, state);
    assert!(conversation.working.is_empty());
    let (offered, then) = match state {
      PanelState::Paused => (ComposerMode::Resume { interrupted: false }, Then::Resume),
      PanelState::Stopped => (ComposerMode::RaiseLimit, Then::RaiseLimit),
      _ => (ComposerMode::Resume { interrupted: true }, Then::ContinueInterrupted),
    };
    let composer = mode(&fixture, task, Recipient::Orchestrator);
    assert_eq!(composer, offered);
    assert_eq!(composer.then(), Some(then));
    // The agent that reads first once the run goes on is whom the composer writes to.
    assert_eq!(conversation.next_agent.as_deref(), Some("Builder"));
    assert_eq!(composer.default_recipient(&conversation), recipients[1].recipient);
    assert_eq!(conversation.entries[0].author, Author::YouTo("Builder".into()));
    let queue = if state == PanelState::Paused {
      Queue::RunResumes(Some("Builder".into()))
    } else {
      Queue::RunContinues(Some("Builder".into()))
    };
    assert_eq!(conversation.entries[0].state, EntryState::Queued(queue));
    assert_eq!(conversation.entries[1].author, Author::YouTo("Reviewer".into()));
    assert_eq!(
      conversation.entries[1].state,
      EntryState::Queued(Queue::AgentNextTurn("Reviewer".into()))
    );
  }
}

#[test]
fn conversation_a_run_stopped_at_a_shared_budget_waits_for_it() {
  for limit in [RunLimit::DailyBudget, RunLimit::GroupBudget] {
    let mut fixture = Fixture::new();
    let task = fixture.task("Add login", at(10));
    let mut run = fixture.started_run(task, at(20));
    let events = run.stop_at_limit(limit, at(21)).expect("stopped");
    fixture.record(&run, &events);
    assert_eq!(
      mode(&fixture, task, Recipient::Orchestrator),
      ComposerMode::Queue(Queued::SharedBudget(limit))
    );
  }
}
