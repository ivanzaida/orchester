//! A question goes to Implement as soon as it is asked, to be answered in a read-only turn the runner drives: it is
//! kept asked, then answering while Implement's words and activity stream in, then answered, stopped with what was
//! written, or *Not asked* with Orchester's reason; *Ask again* continues the thread once an answer ended. The run
//! keeps waiting for review throughout.

use orchester_core::{
  orchestrator::TurnFailure,
  review::{
    AgentWords, QuestionState, ReviewError, ReviewFailure, ReviewMessage, ReviewThread, Said, ThreadKind, ThreadState,
  },
  run::RunStatus,
};
use orchester_exec::programs::Programs;
use orchester_ipc::{Command, Rejection};
use orchester_workspace::{
  runner::{RequestError, RunRequestError},
  tasks::review_threads::{ReviewRefusal, ask, failure_message, read::review_of},
};

use super::support::{Repo, at, body};

fn state(repo: &Repo) -> ThreadState {
  review_of(repo.root(), &Programs::processes(), repo.run)
    .expect("review")
    .threads[0]
    .state
    .clone()
}

/// The runner's turn moves the question from `from` to `to`, as it does while it answers.
fn turn(repo: &Repo, question: &ask::Asking, from: &Said, to: QuestionState) {
  let changed = repo
    .db()
    .change_review_message(question.question, |said| (said == from).then_some(Said::Question(to)))
    .expect("kept");
  assert!(changed, "the question said {from:?}");
}

#[test]
fn review_threads_a_question_is_asked_at_once_and_its_answer_streams_in() {
  let repo = Repo::new();
  let asking = ask::start(repo.root(), repo.run, repo.anchor(2, 2), &body("Why two?"), at(20)).expect("asked");
  assert_eq!(state(&repo), ThreadState::Asked);
  assert_eq!(
    asking.command(),
    Command::AnswerReviewQuestion {
      run: repo.run,
      step: asking.step.clone(),
      message: asking.question,
    }
  );
  assert_eq!(asking.step.as_str(), "implement");
  assert_eq!(state(&repo), ThreadState::Asked, "nothing from Implement yet");

  turn(
    &repo,
    &asking,
    &Said::Question(QuestionState::Asked),
    QuestionState::Answering,
  );
  ask::sent(repo.root(), &asking, Ok(())).expect("the runner took it");
  assert_eq!(state(&repo), ThreadState::Answering, "the runner keeps it answering");
  let words = ReviewMessage::agent(String::new(), true, at(21));
  let mut db = repo.db();
  db.add_review_message(asking.thread, &words).expect("words");
  let writing = Said::Agent(AgentWords {
    writing: true,
    activity: Some("Read src/lib.rs".to_owned()),
  });
  db.write_review_words(words.id(), "Two keeps", &writing)
    .expect("streamed");
  assert_eq!(state(&repo), ThreadState::Answering);
  let read = review_of(repo.root(), &Programs::processes(), repo.run).expect("review");
  let streamed = &read.threads[0].thread.messages()[1];
  assert_eq!(streamed.body(), "Two keeps");
  assert_eq!(streamed.said(), &writing);

  let done = Said::Agent(AgentWords {
    writing: false,
    activity: None,
  });
  db.write_review_words(words.id(), "Two keeps the order.", &done)
    .expect("written");
  turn(
    &repo,
    &asking,
    &Said::Question(QuestionState::Answering),
    QuestionState::Answered,
  );
  assert_eq!(state(&repo), ThreadState::Answered);
  assert_eq!(
    *repo.stored().status(),
    RunStatus::Succeeded,
    "the run still waits for review"
  );
  assert_eq!(
    repo.stored().last_checkpoint().map(|checkpoint| checkpoint.revision()),
    Some(&repo.head),
    "no code changed"
  );
}

#[test]
fn review_threads_ask_again_continues_the_thread_and_a_stopped_answer_keeps_its_words() {
  let repo = Repo::new();
  let asking = ask::start(repo.root(), repo.run, repo.anchor(2, 2), &body("Why two?"), at(20)).expect("asked");
  turn(
    &repo,
    &asking,
    &Said::Question(QuestionState::Asked),
    QuestionState::Answering,
  );
  let words = ReviewMessage::agent("Two keeps the order.".to_owned(), false, at(21));
  repo.db().add_review_message(asking.thread, &words).expect("words");
  turn(
    &repo,
    &asking,
    &Said::Question(QuestionState::Answering),
    QuestionState::Answered,
  );

  let again = ask::again(repo.root(), asking.thread, &body("And three?"), at(30)).expect("asked again");
  assert_eq!(again.thread, asking.thread, "the same thread goes on");
  assert_eq!(state(&repo), ThreadState::Asked);
  assert_eq!(
    ask::again(repo.root(), asking.thread, &body("Hello?"), at(31)),
    Err(ReviewRefusal::Review(ReviewError::NotAnswered))
  );
  turn(
    &repo,
    &again,
    &Said::Question(QuestionState::Asked),
    QuestionState::Answering,
  );
  turn(
    &repo,
    &again,
    &Said::Question(QuestionState::Answering),
    QuestionState::Stopped,
  );
  assert_eq!(state(&repo), ThreadState::AnswerStopped);
  let read = review_of(repo.root(), &Programs::processes(), repo.run).expect("review");
  let prompt = orchester_core::review::question_text(&read.threads[0].thread, read.threads[0].thread.latest_ask());
  assert!(
    prompt.contains("Earlier in this thread") && prompt.contains("Two keeps the order."),
    "{prompt}"
  );
  assert!(
    prompt.contains("read-only") && prompt.ends_with("Question: And three?"),
    "{prompt}"
  );
}

#[test]
fn review_threads_a_question_the_runner_does_not_take_is_not_asked_with_why() {
  let repo = Repo::new();
  let asking = ask::start(repo.root(), repo.run, repo.anchor(1, 1), &body("Why one?"), at(20)).expect("asked");
  let refused = ask::sent(repo.root(), &asking, Err(RunRequestError::Request(RequestError::Lost))).expect_err("lost");
  assert_eq!(refused, ReviewRefusal::NotAsked(ReviewFailure::Lost));
  assert_eq!(state(&repo), ThreadState::NotAsked(ReviewFailure::Lost));

  let again = ask::again(repo.root(), asking.thread, &body("Why one?"), at(21)).expect("asked again");
  let refused = ask::sent(
    repo.root(),
    &again,
    Err(RunRequestError::Rejected(Rejection::ShuttingDown)),
  )
  .expect_err("refused");
  let failure = ReviewFailure::Refused {
    reason: "shutting_down".to_owned(),
  };
  assert_eq!(refused, ReviewRefusal::NotAsked(failure.clone()));
  assert_eq!(state(&repo), ThreadState::NotAsked(failure));

  let third = ask::again(repo.root(), asking.thread, &body("Why one?"), at(22)).expect("asked again");
  let interrupted = ReviewFailure::Turn(TurnFailure::Interrupted);
  turn(
    &repo,
    &third,
    &Said::Question(QuestionState::Asked),
    QuestionState::NotAsked {
      failure: interrupted.clone(),
    },
  );
  assert_eq!(state(&repo), ThreadState::NotAsked(interrupted.clone()));
  assert_eq!(failure_message(&interrupted).key, "review.failure.interrupted");
}

#[test]
fn review_threads_a_question_whose_hand_off_was_cut_off_is_not_asked_and_may_be_asked_again() {
  let repo = Repo::new();
  // Kept asked by an app that ended before the runner took it: no hand-off of this process holds it.
  let (anchor, head) = repo.anchor(2, 2);
  let thread = ReviewThread::start(
    repo.run,
    (ThreadKind::Question, anchor, head),
    &body("Why two?"),
    at(20),
  );
  repo.db().add_review_thread(&thread).expect("kept");
  assert_eq!(state(&repo), ThreadState::NotAsked(ReviewFailure::Lost));

  let again = ask::again(repo.root(), thread.id(), &body("Why two?"), at(21)).expect("asked again");
  assert_eq!(state(&repo), ThreadState::Asked, "its own hand-off is under way");
  turn(
    &repo,
    &again,
    &Said::Question(QuestionState::Asked),
    QuestionState::Answering,
  );
  ask::sent(repo.root(), &again, Ok(())).expect("the runner took it");
  assert_eq!(state(&repo), ThreadState::Answering);
}

#[test]
fn review_threads_no_question_is_asked_of_a_run_that_does_not_wait_for_review() {
  let repo = Repo::new();
  let note = orchester_core::run::ReviewNote::new("Go on.").expect("note");
  repo.return_to_implement(&note, at(20));
  assert_eq!(
    ask::start(repo.root(), repo.run, repo.anchor(1, 1), &body("Why one?"), at(21)).map(|asking| asking.question),
    Err(ReviewRefusal::NotWaiting)
  );
}
