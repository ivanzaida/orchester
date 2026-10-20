//! When the base branch moves (ORC-54, `orchester_core::base_move`): moves within two minutes of the first are one
//! move, and a branch's first sighting starts no window; a commit subject names a task by its key as a word of its
//! own; a notice is deliverable once pending past its window; and the notice an invocation delivers reads in a new
//! session's task part, first in a session resumed after its turn, after the continue prompt in one that was
//! stopped, and after the messages it also delivers.

use orchester_core::{
  base_move::{
    BaseHead, BaseNotice, COALESCE_MILLIS, LandedCommit, LandedTask, NoticeDelivery, Observed, deliver_after,
    landed_tasks,
  },
  conversation::{Message, MessageBody, Recipient},
  id::{InvocationId, NoticeId, RunId, StepExecutionId, TaskId},
  resource::InjectionEvent,
  revision::Revision,
  run::{BaseMoved, BranchName, CONTINUE_PROMPT, DeliveredMessages, MessageOccasion, PromptInput, notice_text},
  time::Timestamp,
};

use crate::{
  fixture::task,
  prompt::{Prompt, builder, step_with},
};

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn revision(digit: char) -> Revision {
  Revision::new(&digit.to_string().repeat(40)).expect("revision")
}

fn notice(deliver: i64) -> BaseNotice {
  BaseNotice {
    id: NoticeId::generate(),
    run: RunId::generate(),
    task: TaskId::generate(),
    branch: BranchName::new("main").expect("branch"),
    revision: revision('b'),
    commits: vec![LandedCommit {
      revision: revision('b'),
      subject: "Merge ORC-7 run #3".to_owned(),
    }],
    tasks: vec![LandedTask {
      key: "ORC-7".to_owned(),
      title: "Pull the people's change".to_owned(),
    }],
    overlapping: vec!["src/login.rs".to_owned()],
    noticed_at: at(10),
    deliver_after: at(deliver),
    delivery: NoticeDelivery::Pending,
  }
}

#[test]
fn base_moved_moves_within_the_window_of_the_first_are_one_and_a_first_sighting_starts_none() {
  let first = BaseHead::observe(None, revision('a'), at(1_000));
  let Observed::First(seen) = first else {
    panic!("the first sighting is no move: {first:?}");
  };
  assert_eq!(seen.moved_at, None);
  let moved = BaseHead::observe(Some(&seen), revision('b'), at(2_000));
  let Observed::Moved { head, moved_at } = moved else {
    panic!("a new head is a move: {moved:?}");
  };
  assert_eq!(moved_at, at(2_000), "not the window of the first sighting");
  assert_eq!(
    BaseHead::observe(Some(&head), revision('b'), at(3_000)),
    Observed::Unchanged
  );
  let within = BaseHead::observe(Some(&head), revision('c'), at(2_000 + COALESCE_MILLIS - 1));
  assert!(
    matches!(within, Observed::Moved { moved_at, .. } if moved_at == at(2_000)),
    "{within:?}"
  );
  let after = BaseHead::observe(Some(&head), revision('c'), at(2_000 + COALESCE_MILLIS));
  assert!(
    matches!(after, Observed::Moved { moved_at, .. } if moved_at == at(2_000 + COALESCE_MILLIS)),
    "{after:?}"
  );
  assert_eq!(deliver_after(at(2_000)), at(2_000 + COALESCE_MILLIS));
}

#[test]
fn base_moved_a_subject_names_a_task_by_its_key_as_a_word_of_its_own() {
  let commits = [
    LandedCommit {
      revision: revision('a'),
      subject: "Merge ORC-2 run #1".to_owned(),
    },
    LandedCommit {
      revision: revision('b'),
      subject: "ORC-21: Implement (run 4)".to_owned(),
    },
  ];
  let tasks = [("ORC-2", "Sign in"), ("ORC-21", "Sign out"), ("ORC-3", "Reset")]
    .map(|(key, title)| (key.to_owned(), title.to_owned()));
  let landed: Vec<String> = landed_tasks(&commits, tasks).into_iter().map(|task| task.key).collect();
  assert_eq!(landed, ["ORC-2", "ORC-21"]);
}

#[test]
fn base_moved_a_notice_is_deliverable_once_pending_past_its_window() {
  let mut waiting = notice(100);
  assert!(!waiting.deliverable(at(99)));
  assert!(waiting.deliverable(at(100)));
  waiting.delivery = NoticeDelivery::Delivered {
    execution: StepExecutionId::generate(),
    invocation: InvocationId::generate(),
    at: at(101),
  };
  assert!(!waiting.deliverable(at(200)), "delivered once");
}

#[test]
fn base_moved_the_notice_names_the_revision_what_landed_the_overlaps_and_how_to_merge() {
  let text = notice_text(&notice(0));
  assert!(
    text.starts_with(&format!("Orchester: the base branch `main` moved to {}", revision('b'))),
    "{text}"
  );
  assert!(text.contains("- ORC-7: Pull the people's change"), "{text}");
  assert!(text.contains("- bbbbbbbbbb Merge ORC-7 run #3"), "{text}");
  assert!(text.contains("- `src/login.rs`"), "{text}");
  assert!(
    text.contains("`git merge main`; never `git stash`, never rebase a shared branch"),
    "{text}"
  );
  assert!(
    text.contains("say in your result what you merged and how you resolved the conflicts"),
    "{text}"
  );
}

fn assembled(moved: BaseMoved<'_>, messages: Option<DeliveredMessages<'_>>, event: InjectionEvent) -> String {
  let prompt = Prompt::new(builder("Role."), step_with("Step."), task("Brief.", &[]));
  PromptInput {
    base_moved: Some(moved),
    messages,
    ..prompt.input(&[], &[], true)
  }
  .assemble(event)
  .expect("no variables")
  .text
}

#[test]
fn base_moved_an_invocation_reads_the_notice_where_its_event_puts_it() {
  let notice = notice(0);
  let after_turn = BaseMoved {
    notice: &notice,
    after_turn: true,
  };
  let start = assembled(after_turn, None, InjectionEvent::Start);
  let heading = start.find("# The base branch moved").expect("in the task part");
  assert!(heading > start.find("# Task ").expect("the task part"), "{start}");
  assert!(
    heading < start.find("# When you finish").expect("the result"),
    "{start}"
  );

  let resumed = assembled(after_turn, None, InjectionEvent::Resume);
  assert!(
    resumed.starts_with("Orchester: the base branch `main` moved to"),
    "{resumed}"
  );
  assert!(!resumed.contains(CONTINUE_PROMPT), "it finished its turn: {resumed}");
  let stopped = BaseMoved {
    notice: &notice,
    after_turn: false,
  };
  let continued = assembled(stopped, None, InjectionEvent::Resume);
  assert!(continued.starts_with(CONTINUE_PROMPT), "{continued}");
  assert!(
    continued.contains("Orchester: the base branch `main` moved to"),
    "{continued}"
  );

  let messages = [Message::from_person(
    TaskId::generate(),
    None,
    Recipient::Orchestrator,
    MessageBody::new("Keep email sign-in.").expect("a message"),
    at(5),
  )];
  let delivered = DeliveredMessages {
    occasion: MessageOccasion::AfterTurn,
    messages: &messages,
  };
  let both = assembled(after_turn, Some(delivered), InjectionEvent::Resume);
  let wrote = both.find("Keep email sign-in.").expect("the message");
  let told = both.find("Orchester: the base branch").expect("the notice");
  assert!(wrote < told, "the person's messages first: {both}");
}
