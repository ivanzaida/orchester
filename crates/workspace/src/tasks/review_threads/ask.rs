//! *Ask Implement*: a question goes to the run's implementing step's agent as soon as the person asks it, and is
//! answered in a read-only turn the runner drives (`orchester-runner`'s `execution::review`), streamed into the
//! thread. The question is kept asked first ([`start`], [`again`]), then handed to the project's runner ([`ask`]),
//! which keeps it answering as it takes it; one the runner did not take is kept *Not asked* with Orchester's reason
//! ([`sent`]), as is one whose hand-off was cut off part-way (`recover`), and nothing else changes. *Stop answering*
//! ends the turn and keeps what it wrote ([`stop`]). The run keeps waiting for review throughout.

use std::path::Path;

use orchester_core::{
  conversation::MessageBody,
  id::{ReviewMessageId, ReviewThreadId, RunId},
  pull_request::feedback::LineAnchor,
  resource::StepKey,
  review::{QuestionState, ReviewError, ReviewThread, Said, ThreadKind},
  revision::Revision,
  time::Timestamp,
};
use orchester_ipc::Command;
use orchester_store::ProjectDb;

use super::{
  ReviewRefusal,
  recover::{self, Holding},
  send::failure_of,
  thread_in, waiting,
};
use crate::{
  messages::Message,
  runner::{RunRequestError, Runners},
  tasks::{detail::TaskDetail, return_work},
  workspace::OpenProject,
};

/// A question kept asked, to be handed to the project's runner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asking {
  pub run: RunId,
  pub thread: ReviewThreadId,
  pub question: ReviewMessageId,
  /// The run's implementing step, whose agent answers.
  pub step: StepKey,
}

impl Asking {
  /// What the runner is asked.
  pub fn command(&self) -> Command {
    Command::AnswerReviewQuestion {
      run: self.run,
      step: self.step.clone(),
      message: self.question,
    }
  }
}

/// Starts a question thread on `run` of the project at `root`, about `anchor` of its diff at `head`, with the person's
/// `body` at `now`, kept asked, to be handed to the runner with [`ask`].
pub fn start(
  root: &Path,
  run: RunId,
  (anchor, head): (LineAnchor, Revision),
  body: &MessageBody,
  now: Timestamp,
) -> Result<Asking, ReviewRefusal> {
  let mut db = ProjectDb::open(root)?;
  let step = implementing_step(root, &db, run)?;
  let thread = ReviewThread::start(run, (ThreadKind::Question, anchor, head), body, now);
  let question = thread.latest_ask().id();
  let holding = Holding::hold(vec![question]);
  db.add_review_thread(&thread)?;
  holding.keep(&[question]);
  Ok(Asking {
    run,
    thread: thread.id(),
    question,
    step,
  })
}

/// *Ask again* in question thread `thread`, once its last answer ended or it was not asked: the person's `body` at
/// `now`, kept asked, to be handed to the runner with [`ask`].
pub fn again(root: &Path, thread: ReviewThreadId, body: &MessageBody, now: Timestamp) -> Result<Asking, ReviewRefusal> {
  let mut db = ProjectDb::open(root)?;
  let stored = thread_in(&db, thread)?;
  if stored.kind() != ThreadKind::Question {
    return Err(ReviewRefusal::Review(ReviewError::WrongKind));
  }
  let step = implementing_step(root, &db, stored.run())?;
  recover::threads(&mut db, stored.run())?;
  let stored = thread_in(&db, thread)?;
  let question = stored.follow_up(body, now).map_err(ReviewRefusal::Review)?;
  let holding = Holding::hold(vec![question.id()]);
  db.add_review_message(thread, &question)?;
  holding.keep(&[question.id()]);
  Ok(Asking {
    run: stored.run(),
    thread,
    question: question.id(),
    step,
  })
}

/// Hands `asking` to the runner of `open`, starting it when the project has none, and keeps how that went. Blocks on
/// SQLite once the runner answered. Cut off part-way, the question is recovered when the run's review is read next.
pub async fn ask(runners: &Runners, open: &OpenProject, asking: &Asking) -> Result<(), ReviewRefusal> {
  let _holding = Holding::held(vec![asking.question]);
  let result = runners.control_run_in(open, asking.command()).await;
  sent(&open.repo_root, asking, result)
}

/// Keeps how handing `asking` to the runner went: a question the runner did not take is *Not asked*, with why, which is
/// the refusal. The runner keeps a question it took answering until its turn ends.
pub fn sent(root: &Path, asking: &Asking, result: Result<(), RunRequestError>) -> Result<(), ReviewRefusal> {
  let _holding = Holding::held(vec![asking.question]);
  let Err(error) = result else {
    return Ok(());
  };
  tracing::info!(%error, run = %asking.run, "the runner did not take a review question");
  let failure = failure_of(&error);
  let not_asked = Said::Question(QuestionState::NotAsked {
    failure: failure.clone(),
  });
  ProjectDb::open(root)?.change_review_message(asking.question, |said| {
    matches!(said, Said::Question(QuestionState::Asked)).then(|| not_asked.clone())
  })?;
  Err(ReviewRefusal::NotAsked(failure))
}

/// *Stop answering* in `thread` of `run`: the runner ends the answer's turn, and what it wrote stays.
pub async fn stop(
  runners: &Runners,
  open: &OpenProject,
  (run, thread): (RunId, ReviewThreadId),
) -> Result<(), Message> {
  runners
    .control_run_in(open, Command::StopReviewAnswer { run, thread })
    .await
    .map_err(|error| crate::tasks::run::control_message(&error))
}

/// The implementing step of `run`, which must wait for review, in the project at `root` whose database is `db`.
fn implementing_step(root: &Path, db: &ProjectDb, run: RunId) -> Result<StepKey, ReviewRefusal> {
  let stored = waiting(db, run)?;
  let detail = TaskDetail::load_in(root, stored.task())?;
  let view = detail
    .run
    .as_ref()
    .filter(|view| view.id() == run)
    .ok_or(ReviewRefusal::NotWaiting)?;
  return_work::implementing_step(view).ok_or(ReviewRefusal::NoImplementingStep)
}
