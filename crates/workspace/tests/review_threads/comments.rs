//! Comments wait unsent in the tray (editable, deletable) until *Send to Implement*, which sends every one of them
//! and no question in one return of the run's work quoting each; they are sent while Implement works, addressed in the
//! commit that changed their lines with Implement's words once its work ended, and *Reply* reopens an addressed one
//! as a new unsent comment. A send the runner does not take keeps every comment unsent with why, and a send cut off
//! part-way is recovered: sent when the run went back to Implement, else unsent again.

use orchester_core::{
  review::{CommentState, ReviewFailure, ReviewMessage, ReviewThread, Said, ThreadState},
  run::{Answerer, ReviewNote},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_ipc::{Command, WorkReturn};
use orchester_workspace::{
  runner::{RequestError, RunRequestError},
  tasks::{
    resolve::RunWorktrees,
    review_threads::{
      self, ReviewRefusal,
      read::{Review, review_of},
      send,
    },
  },
};

use super::support::{Repo, at, body};

fn review(repo: &Repo) -> Review {
  review_of(repo.root(), &Programs::processes(), repo.run).expect("review")
}

fn states(review: &Review) -> Vec<ThreadState> {
  review.threads.iter().map(|view| view.state.clone()).collect()
}

#[test]
fn review_threads_comments_wait_unsent_and_change_or_go_until_sent() {
  let repo = Repo::new();
  let first =
    review_threads::comment(repo.root(), repo.run, repo.anchor(1, 1), &body("Rename one."), at(20)).expect("kept");
  let second =
    review_threads::comment(repo.root(), repo.run, repo.anchor(2, 3), &body("Drop these."), at(21)).expect("kept");
  let read = review(&repo);
  assert_eq!(states(&read), vec![ThreadState::NotSent, ThreadState::NotSent]);
  assert_eq!(read.unsent(), 2);
  assert_eq!(read.head.as_ref(), Some(&repo.head));

  review_threads::edit(repo.root(), first.latest_ask().id(), &body("Rename one to first.")).expect("edited");
  review_threads::delete(repo.root(), second.latest_ask().id()).expect("deleted");
  let read = review(&repo);
  assert_eq!(read.threads.len(), 1, "the thread of its only comment went with it");
  assert_eq!(read.threads[0].thread.latest_ask().body(), "Rename one to first.");

  review_threads::viewed(repo.root(), (repo.run, &repo.head, "src/lib.rs"), Some(at(22))).expect("viewed");
  let read = review(&repo);
  assert_eq!(read.viewed.len(), 1);
  assert_eq!(read.viewed[0].path, "src/lib.rs");
  review_threads::viewed(repo.root(), (repo.run, &repo.head, "src/lib.rs"), None).expect("not viewed");
  assert!(review(&repo).viewed.is_empty());
}

#[test]
fn review_threads_send_to_implement_sends_every_comment_and_no_question_in_one_quoting_note() {
  let repo = Repo::new();
  let comment =
    review_threads::comment(repo.root(), repo.run, repo.anchor(1, 1), &body("Rename one."), at(20)).expect("kept");
  review_threads::comment(
    repo.root(),
    repo.run,
    repo.anchor(3, 3),
    &body("Document three."),
    at(21),
  )
  .expect("kept");
  let question =
    review_threads::ask::start(repo.root(), repo.run, repo.anchor(2, 2), &body("Why two?"), at(22)).expect("asked");

  let sending = send::begin(
    repo.root(),
    &RunWorktrees::here_for_test(repo.root(), Programs::processes()),
    repo.run,
  )
  .expect("sending");
  assert_eq!(sending.comments.len(), 2, "questions never go with the comments");
  let Command::ReturnRun(WorkReturn { step, note, by, .. }) = &sending.command else {
    panic!("the run's work returns to Implement: {:?}", sending.command);
  };
  assert_eq!(step.as_str(), "implement");
  assert_eq!(*by, Answerer::Person);
  let note = note.as_str();
  assert!(note.contains("2 comments"), "{note}");
  assert!(
    note.contains("src/lib.rs, line 1 at") && note.contains("Rename one."),
    "{note}"
  );
  assert!(
    note.contains("src/lib.rs, line 3 at") && note.contains("Document three."),
    "{note}"
  );
  assert!(!note.contains("Why two?"), "{note}");
  let read = review(&repo);
  assert_eq!(read.threads[0].state, ThreadState::Sending);
  assert_eq!(read.threads[2].state, ThreadState::Asked, "{:?}", question);

  send::sent(repo.root(), &sending, Ok(()), at(30)).expect("sent");
  let Command::ReturnRun(WorkReturn { note, .. }) = &sending.command else {
    unreachable!("checked above");
  };
  repo.return_to_implement(note, at(31));
  let read = review(&repo);
  assert_eq!(read.threads[0].state, ThreadState::Sent);
  assert!(read.implement_at_work);
  assert_eq!(read.unsent(), 0);

  let commit = repo.implement(
    "fn first() {}\nfn two() {}\nfn three() {}\n/// Three.\n",
    "Renamed one to first.",
    at(40),
  );
  let read = review(&repo);
  assert_eq!(read.threads[0].state, ThreadState::Addressed { commit: Some(commit) });
  assert!(!read.implement_at_work);
  let words: &ReviewMessage = read.threads[0].thread.messages().last().expect("Implement's words");
  assert!(matches!(words.said(), Said::Agent(agent) if !agent.writing));
  assert_eq!(words.body(), "Renamed one to first.");
  assert_eq!(
    read.threads[1].state,
    ThreadState::Addressed { commit: None },
    "no commit changed its line"
  );

  review_threads::reply(repo.root(), comment.id(), &body("Also rename two."), at(50)).expect("replied");
  let read = review(&repo);
  assert_eq!(read.threads[0].state, ThreadState::NotSent, "a reply waits in the tray");
  assert_eq!(read.threads[0].thread.messages().len(), 3);
  assert_eq!(read.unsent(), 1);
}

#[test]
fn review_threads_a_send_the_runner_does_not_take_keeps_every_comment_unsent_with_why() {
  let repo = Repo::new();
  review_threads::comment(repo.root(), repo.run, repo.anchor(1, 1), &body("Rename one."), at(20)).expect("kept");
  review_threads::comment(repo.root(), repo.run, repo.anchor(2, 2), &body("Rename two."), at(21)).expect("kept");
  let sending = send::begin(
    repo.root(),
    &RunWorktrees::here_for_test(repo.root(), Programs::processes()),
    repo.run,
  )
  .expect("sending");
  let refused = send::sent(
    repo.root(),
    &sending,
    Err(RunRequestError::Request(RequestError::NotConnected)),
    Timestamp::now(),
  )
  .expect_err("not sent");
  assert_eq!(refused, ReviewRefusal::NotSent(ReviewFailure::RunnerUnavailable));
  assert_eq!(refused.message().key, "review.failure.runnerUnavailable");
  let read = review(&repo);
  assert_eq!(
    states(&read),
    vec![
      ThreadState::CouldNotSend(ReviewFailure::RunnerUnavailable),
      ThreadState::CouldNotSend(ReviewFailure::RunnerUnavailable),
    ]
  );
  assert_eq!(read.unsent(), 2, "nothing is lost: both wait to be sent again");
  assert!(!read.implement_at_work);
  assert_eq!(
    send::begin(
      repo.root(),
      &RunWorktrees::here_for_test(repo.root(), Programs::processes()),
      repo.run
    )
    .expect("sent again")
    .comments
    .len(),
    2
  );
}

/// Keeps `comment` sending since `since` with the run's attempts so far, as an app that ended before the runner
/// answered left it: no hand-off of this process holds it.
fn cut_off(repo: &Repo, comment: &ReviewThread, since: Timestamp) {
  let sending = Said::Comment(CommentState::Sending {
    since,
    attempts: repo.stored().attempts().len(),
  });
  let changed = repo
    .db()
    .change_review_message(comment.latest_ask().id(), |_| Some(sending.clone()))
    .expect("kept");
  assert!(changed);
}

#[test]
fn review_threads_a_send_cut_off_before_the_runner_took_it_waits_unsent_again() {
  let repo = Repo::new();
  let comment =
    review_threads::comment(repo.root(), repo.run, repo.anchor(1, 1), &body("Rename one."), at(20)).expect("kept");
  cut_off(&repo, &comment, at(30));
  let read = review(&repo);
  assert_eq!(states(&read), vec![ThreadState::CouldNotSend(ReviewFailure::Lost)]);
  assert_eq!(read.unsent(), 1, "nothing is lost");
  assert!(!read.implement_at_work, "approval is open again");
  review_threads::edit(repo.root(), comment.latest_ask().id(), &body("Rename one to first.")).expect("editable");
  assert_eq!(
    send::begin(
      repo.root(),
      &RunWorktrees::here_for_test(repo.root(), Programs::processes()),
      repo.run
    )
    .expect("sent again")
    .comments,
    vec![comment.latest_ask().id()]
  );
  assert_eq!(
    review(&repo).threads[0].state,
    ThreadState::Sending,
    "its own hand-off is under way"
  );
}

#[test]
fn review_threads_a_send_cut_off_after_the_runner_took_it_is_sent_and_then_addressed() {
  let repo = Repo::new();
  let comment =
    review_threads::comment(repo.root(), repo.run, repo.anchor(1, 1), &body("Rename one."), at(20)).expect("kept");
  cut_off(&repo, &comment, at(30));
  repo.return_to_implement(&ReviewNote::new("Rename one.").expect("note"), at(31));
  let read = review(&repo);
  assert_eq!(states(&read), vec![ThreadState::Sent]);
  assert!(read.implement_at_work);

  let commit = repo.implement("fn first() {}\nfn two() {}\nfn three() {}\n", "Renamed one.", at(40));
  let read = review(&repo);
  assert_eq!(states(&read), vec![ThreadState::Addressed { commit: Some(commit) }]);
  assert_eq!(
    read.threads[0].thread.messages().last().map(ReviewMessage::body),
    Some("Renamed one.")
  );
}

#[test]
fn review_threads_resolved_threads_fold_and_a_later_head_outdates_changed_lines() {
  let repo = Repo::new();
  let resolved =
    review_threads::comment(repo.root(), repo.run, repo.anchor(3, 3), &body("Fine."), at(20)).expect("kept");
  review_threads::comment(repo.root(), repo.run, repo.anchor(1, 1), &body("Rename one."), at(21)).expect("kept");
  review_threads::resolve(repo.root(), resolved.id(), Some(at(22))).expect("resolved");
  let changed = repo.commit("fn uno() {}\nfn two() {}\nfn three() {}\n");
  let read = review(&repo);
  assert_eq!(read.threads[0].state, ThreadState::Resolved);
  assert_eq!(read.threads[1].state, ThreadState::Outdated { commit: changed });
  assert!(read.threads.iter().all(|view| view.state.folds()));
  assert_eq!(read.threads.len(), 2, "resolved threads are kept");

  review_threads::resolve(repo.root(), resolved.id(), None).expect("unfolded");
  assert_eq!(review(&repo).threads[0].state, ThreadState::NotSent);
}

#[test]
fn review_threads_nothing_to_send_and_only_unsent_comments_change() {
  let repo = Repo::new();
  assert_eq!(
    send::begin(
      repo.root(),
      &RunWorktrees::here_for_test(repo.root(), Programs::processes()),
      repo.run
    )
    .map(|sending| sending.comments),
    Err(ReviewRefusal::NothingToSend)
  );
  let comment =
    review_threads::comment(repo.root(), repo.run, repo.anchor(1, 1), &body("Rename one."), at(20)).expect("kept");
  let sending = send::begin(
    repo.root(),
    &RunWorktrees::here_for_test(repo.root(), Programs::processes()),
    repo.run,
  )
  .expect("sending");
  assert_eq!(
    review_threads::edit(repo.root(), comment.latest_ask().id(), &body("Changed")),
    Err(ReviewRefusal::Review(orchester_core::review::ReviewError::NotEditable))
  );
  assert_eq!(
    review_threads::delete(repo.root(), comment.latest_ask().id()),
    Err(ReviewRefusal::Review(orchester_core::review::ReviewError::NotEditable))
  );
  assert_eq!(sending.run, repo.run);
}
