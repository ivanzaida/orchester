//! Answering a person's review question on a run that waits for review (`orchester_core::review`, protocol 40): one
//! read-only turn of the agent of the run's implementing step, in the run's worktree, on the step's connection and
//! model ([`turn`]), Claude Code ([`claude`]) or Codex App Server ([`codex`]). A question this runner takes is kept
//! answering at once, so one still asked was never taken (the desktop recovers it). Its words and live activity are
//! kept in the thread as they come, and how it ends with the question; the run itself never changes. A thread answers
//! one question at a time. Stopping a turn ends its CLI, and what it wrote stays. An answer this runner does not drive,
//! when a desktop attaches, was cut short by the runner before it, and is kept as interrupted, its words as written.

mod claude;
mod codex;
mod turn;

use std::{collections::HashMap, sync::Arc};

use orchester_core::{
  id::{ReviewMessageId, ReviewThreadId, RunId},
  orchestrator::{TIME_LIMIT, TurnFailure},
  resource::StepKey,
  review::{AgentWords, QuestionState, ReviewFailure, Said},
  store::RunStore,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::{ProjectDb, StoreError};
use parking_lot::Mutex;
use tokio::{sync::oneshot, task::JoinHandle};

use self::turn::ReviewTurn;
use super::{Scheduler, failed, idle::check_daily_budget};

/// The answers under way, by thread.
#[derive(Default)]
pub(super) struct Reviews {
  running: Mutex<HashMap<ReviewThreadId, Running>>,
}

struct Running {
  question: ReviewMessageId,
  /// Ends the turn's CLI; gone once it was used.
  stop: Option<oneshot::Sender<()>>,
  task: JoinHandle<()>,
}

impl Reviews {
  /// The questions whose answers are under way.
  fn held(&self) -> Vec<ReviewMessageId> {
    let mut running = self.running.lock();
    running.retain(|_, turn| !turn.task.is_finished());
    running.values().map(|turn| turn.question).collect()
  }
}

impl Scheduler {
  /// Whether no turn of the Orchestrator chat and no review answer is under way.
  pub(super) fn turns_idle(&self) -> bool {
    self.inner.chats.idle() && self.inner.reviews.held().is_empty()
  }

  /// Answers review question `message` on `run` with a turn of the agent of its implementing step `step`; nothing when
  /// its answer is under way or ended already. The question is answering once this returns: one the desktop still
  /// finds asked was never taken.
  pub async fn answer_review_question(
    &self,
    run: RunId,
    step: StepKey,
    message: ReviewMessageId,
  ) -> Result<(), Rejection> {
    if self.inner.board.is_stopping() {
      return Err(Rejection::ShuttingDown);
    }
    let mut db = self.open()?;
    let thread = db
      .review_thread_of(message)
      .map_err(|error| failed("cannot read the review thread", &error))?
      .filter(|thread| thread.run() == run)
      .ok_or(Rejection::QuestionNotAwaited)?;
    match thread.message(message).map(|question| question.said()) {
      Some(Said::Question(QuestionState::Asked)) => {}
      Some(Said::Question(_)) => return Ok(()),
      _ => return Err(Rejection::QuestionNotAwaited),
    }
    // An answer's tokens count toward the daily budget: a used one refuses it as it refuses a run.
    check_daily_budget(&db)?;
    let stored = db
      .run(run)
      .map_err(|error| failed("cannot read the run", &error))?
      .ok_or(Rejection::RunNotFound)?;
    let mut running = self.inner.reviews.running.lock();
    running.retain(|_, turn| !turn.task.is_finished());
    if let Some(turn) = running.get(&thread.id()) {
      return if turn.question == message {
        Ok(())
      } else {
        Err(Rejection::QuestionNotAwaited)
      };
    }
    let answering = Said::Question(QuestionState::Answering);
    let taken = db
      .change_review_message(message, |said| {
        matches!(said, Said::Question(QuestionState::Asked)).then(|| answering.clone())
      })
      .map_err(|error| failed("cannot keep the review question answering", &error))?;
    if !taken {
      return Err(Rejection::QuestionNotAwaited);
    }
    self.review_changed(&db, run);
    let (stop, stopped) = oneshot::channel();
    let id = thread.id();
    let turn = ReviewTurn {
      root: self.inner.project_root.clone(),
      notifier: self.inner.notifier.clone(),
      vault: Arc::clone(&self.inner.vault),
      run: stored,
      thread,
      question: message,
      step,
      time_limit: TIME_LIMIT,
    };
    let task = tokio::spawn(turn.run(stopped));
    running.insert(
      id,
      Running {
        question: message,
        stop: Some(stop),
        task,
      },
    );
    Ok(())
  }

  /// Stops the answer under way in `thread` of `run`. A question the thread still shows answered that this runner does
  /// not answer is kept stopped: nothing would end it otherwise.
  pub fn stop_review_answer(&self, run: RunId, thread: ReviewThreadId) -> Result<(), Rejection> {
    {
      let mut running = self.inner.reviews.running.lock();
      running.retain(|_, turn| !turn.task.is_finished());
      if let Some(turn) = running.get_mut(&thread) {
        if let Some(stop) = turn.stop.take()
          && stop.send(()).is_err()
        {
          tracing::debug!(%thread, "the review answer had ended before it was stopped");
        }
        return Ok(());
      }
    }
    let mut db = self.open()?;
    let Some(stored) = db
      .review_thread(thread)
      .map_err(|error| failed("cannot read the review thread", &error))?
    else {
      return Ok(());
    };
    if let Some(question) = stored.asking().map(|question| question.id()) {
      let stopped = Said::Question(QuestionState::Stopped);
      let changed = db
        .change_review_message(question, |said| {
          matches!(said, Said::Question(QuestionState::Answering)).then(|| stopped.clone())
        })
        .map_err(|error| failed("cannot keep the stopped review answer", &error))?;
      if changed {
        end_words(&mut db, question).map_err(|error| failed("cannot keep the stopped review answer", &error))?;
      }
      self.review_changed(&db, run);
    }
    Ok(())
  }

  /// Keeps the review answers `db` still has under way, but those this runner answers, as interrupted: no runner
  /// answers them any more.
  pub(super) fn interrupt_review_answers(&self, db: &mut ProjectDb) -> Result<(), Rejection> {
    let held = self.inner.reviews.held();
    let answering = db
      .answering_review_questions()
      .map_err(|error| failed("cannot read the review answers under way", &error))?;
    let interrupted = Said::Question(QuestionState::NotAsked {
      failure: ReviewFailure::Turn(TurnFailure::Interrupted),
    });
    let store = |error: &StoreError| failed("cannot keep an interrupted review answer", error);
    for (run, question) in answering.into_iter().filter(|(_, question)| !held.contains(question)) {
      let changed = db
        .change_review_message(question, |said| {
          matches!(said, Said::Question(QuestionState::Answering)).then(|| interrupted.clone())
        })
        .map_err(|error| store(&error))?;
      if changed {
        end_words(db, question).map_err(|error| store(&error))?;
      }
      self.review_changed(db, run);
    }
    Ok(())
  }

  /// Tells the desktop that a review thread of `run` changed: its run's page reads the threads again.
  fn review_changed(&self, db: &ProjectDb, run: RunId) {
    match db.run(run) {
      Ok(Some(stored)) => self.inner.notifier.send(Event::RunChanged {
        run,
        task: stored.task(),
      }),
      Ok(None) => {}
      Err(error) => tracing::warn!(%run, %error, "cannot read the run whose review changed"),
    }
  }
}

/// Keeps Implement's words after `question` as written, without what it did last: no turn writes them any more.
fn end_words(db: &mut ProjectDb, question: ReviewMessageId) -> Result<(), StoreError> {
  let Some(thread) = db.review_thread_of(question)? else {
    return Ok(());
  };
  let ended = Said::Agent(AgentWords {
    writing: false,
    activity: None,
  });
  let writing = thread
    .messages()
    .iter()
    .skip_while(|message| message.id() != question)
    .filter(|message| matches!(message.said(), Said::Agent(words) if words.writing));
  for words in writing {
    db.write_review_words(words.id(), words.body(), &ended)?;
  }
  Ok(())
}
