//! What the task page reads of a run's delivery besides its state: the queue's lines in the run's conversation and
//! where they sit among its rows, the commits *Review before merging* lists, and *I've resolved it* naming the files it
//! found conflict markers left in.

use std::sync::Arc;

use orchester_core::{
  integration::RunWork,
  merge_queue::{Left, QueueEvent, QueueState},
  revision::Revision,
  run::{StepStatus, TokenUsage},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  merge_queue::{self, QueueRefusal},
  tasks::{
    changes::{RunRange, run_commits_in},
    timeline::{
      Item, ItemKind, RunEnd, StepHead,
      delivery::{self, DeliveryKind},
      view::Row,
    },
  },
};

use super::harness::{Repo, git};

/// A step's divider in the chat, started at `started`.
fn step(number: usize, started: Timestamp) -> Arc<Row> {
  let head = StepHead {
    execution: None,
    number,
    step: "Implement".into(),
    agent: "Builder".into(),
    command: false,
    provider: None,
    vendor: None,
    model: "gpt-5.5".into(),
    invocations: 1,
    started: Some(started),
    ended: Some(started),
    usage: TokenUsage::default(),
    before_edit: None,
    status: Some(StepStatus::Passed),
    attempt: None,
    ended_on: false,
    approved: false,
    tool_run: None,
  };
  Arc::new(Row::Step {
    head,
    key: format!("step-{number}").into(),
    emphasized: false,
    foldable: true,
    matches: None,
  })
}

fn end(at: Timestamp) -> Arc<Row> {
  Arc::new(Row::Item(Item {
    key: "end".into(),
    execution: None,
    at,
    kind: ItemKind::End(RunEnd::Ready),
  }))
}

fn shifted(at: Timestamp, millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(at.unix_millis() + millis)
}

#[test]
fn merge_queue_conversation_lines_follow_the_steps_before_them_and_come_before_later_steps_and_the_end() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "auto"), "orchester/orc-1-run-1", ("shared.txt", "run\n"), 100);
  repo.drive();
  let db = ProjectDb::open(repo.root()).expect("open");
  let delivery = merge_queue::delivery_of(&db, run.id()).expect("read").expect("joined");

  let lines = delivery::lines(&delivery);
  let events: Vec<&QueueEvent> = lines
    .iter()
    .map(|line| match &line.kind {
      DeliveryKind::Event(event) => event,
      DeliveryKind::WaitsForApproval => panic!("an Auto-merge run never waits for approval"),
    })
    .collect();
  assert!(
    matches!(
      events.as_slice(),
      [
        QueueEvent::Joined,
        QueueEvent::Synced { .. },
        QueueEvent::CheckStarted { .. },
        QueueEvent::CheckPassed { .. },
        QueueEvent::Merged { .. },
        QueueEvent::CleanedUp { .. }
      ]
    ),
    "{events:?}"
  );

  let (first, last) = (lines[0].at, lines[lines.len() - 1].at);
  let ran = step(1, shifted(first, -10));
  let later = step(2, shifted(last, 10));
  assert_eq!(
    delivery::places(&[ran.clone(), later, end(shifted(first, -5))], &lines),
    vec![1; lines.len()],
    "after the step that ran before them, before the one that started after them"
  );
  assert_eq!(
    delivery::places(&[ran.clone(), end(shifted(first, -5))], &lines),
    vec![1; lines.len()],
    "before the run's end, which closes the chat"
  );
  assert_eq!(
    delivery::places(&[ran], &lines),
    vec![1; lines.len()],
    "after every row"
  );
}

#[test]
fn merge_queue_conversation_says_a_run_waits_for_approval_last() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded(
    (&task, "approve"),
    "orchester/orc-1-run-1",
    ("shared.txt", "run\n"),
    100,
  );
  repo.drive();
  assert_eq!(repo.state(&run), QueueState::WaitingForApproval);
  let db = ProjectDb::open(repo.root()).expect("open");
  let delivery = merge_queue::delivery_of(&db, run.id()).expect("read").expect("joined");

  let lines = delivery::lines(&delivery);
  assert_eq!(
    lines.last().map(|line| &line.kind),
    Some(&DeliveryKind::WaitsForApproval)
  );
}

#[test]
fn merge_queue_review_lists_the_runs_own_commits_with_the_sync_and_none_of_mains() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded((&task, "approve"), "orchester/orc-1-run-1", ("other.txt", "run\n"), 100);
  Repo::commit_on_main(repo.root(), ("shared.txt", "main's line\n"));
  repo.drive();
  assert_eq!(repo.state(&run), QueueState::WaitingForApproval);

  let checkpoint = Revision::new(&git(&repo.worktree(&run), &["rev-parse", "HEAD~1"])).expect("revision");
  let range = RunRange {
    run: run.id(),
    work: RunWork::of(&run),
    head: Some(checkpoint),
  };
  let commits = run_commits_in(repo.root(), &repo.programs(), &range).expect("listed");

  let subjects: Vec<&str> = commits.iter().map(|commit| commit.subject.as_str()).collect();
  assert_eq!(
    subjects.len(),
    2,
    "the sync's merge commit, then the run's own: {subjects:?}"
  );
  assert_eq!(subjects[1], format!("{}: Implement", task.key()));
  assert!(
    !subjects.contains(&"A change on main"),
    "main's commits are not the run's: {subjects:?}"
  );
}

#[test]
fn merge_queue_resolved_by_hand_names_the_files_with_conflict_markers_left() {
  let mut repo = Repo::new();
  let task = repo.task("Sign in");
  let run = repo.succeeded(
    (&task, "unfit"),
    "orchester/orc-1-run-1",
    ("shared.txt", "the run's line\n"),
    100,
  );
  Repo::commit_on_main(repo.root(), ("shared.txt", "main's line\n"));
  repo.drive();
  merge_queue::resolve_myself(repo.root(), &repo.programs(), run.id()).expect("merged in");

  let refused = merge_queue::resolved(repo.root(), &repo.programs(), run.id());

  assert_eq!(refused, Err(QueueRefusal::MarkersLeft(vec!["shared.txt".to_owned()])));
  let markers = refused
    .err()
    .and_then(|refusal| refusal.markers_left().map(<[String]>::to_vec));
  assert_eq!(markers, Some(vec!["shared.txt".to_owned()]));
  assert!(matches!(repo.state(&run), QueueState::Left(Left::Conflicts { .. })));
}
