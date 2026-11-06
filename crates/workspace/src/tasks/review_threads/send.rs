//! *Send to Implement (N)*: every comment waiting unsent in the run's tray goes at once (questions never do), as one
//! return of the run's work to its writing step — the pipeline's Implement, as *Return the work to an earlier step*
//! returns it ([`return_work`]) — with one note quoting each file, its lines and the comment
//! ([`orchester_core::review::send_note`]); the Conversation shows it as that one message. The comments are kept
//! sending first ([`begin`]), then handed to the project's runner ([`send`]): taken, they are sent and Implement works
//! on them; not taken, every one of them waits unsent again with why ([`sent`]), and nothing is lost. Once the run's
//! steps ran again it waits for review again, and reading the review settles the comments Implement addressed.

use std::path::Path;

use orchester_core::{
  id::{ReviewMessageId, RunId},
  review::{CommentState, ReviewFailure, ReviewMessage, ReviewThread, Said, send_note},
  run::Answerer,
  time::Timestamp,
};
use orchester_ipc::Command;
use orchester_store::ProjectDb;

use super::{
  ReviewRefusal,
  recover::{self, Holding},
  waiting,
};
use crate::{
  runner::{RequestError, RunRequestError, Runners},
  tasks::{
    detail::TaskDetail,
    resolve::{self, RunWorktrees},
    return_work,
  },
  workspace::OpenProject,
};

/// The comments kept sending, and the command that returns the run's work with them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sending {
  pub run: RunId,
  pub comments: Vec<ReviewMessageId>,
  pub command: Command,
}

/// Keeps every comment waiting unsent on `run` of the project at `root` sending, and gives the command that returns the
/// run's work to its implementing step with them, to be handed to the runner with [`send`]. Refused while no comment
/// waits, and as a return of the run's work is refused. The run's worktree is read through `worktrees`.
pub fn begin(root: &Path, worktrees: &RunWorktrees, run: RunId) -> Result<Sending, ReviewRefusal> {
  let mut db = ProjectDb::open(root)?;
  let stored = waiting(&db, run)?;
  let threads = recover::threads(&mut db, run)?;
  let unsent: Vec<(&ReviewThread, &ReviewMessage)> = threads
    .iter()
    .filter(|thread| thread.resolved_at().is_none())
    .flat_map(|thread| thread.unsent().map(move |comment| (thread, comment)))
    .collect();
  let note = send_note(&unsent).ok_or(ReviewRefusal::NothingToSend)?;
  let detail = TaskDetail::load_in(root, stored.task())?;
  let view = detail
    .run
    .as_ref()
    .filter(|view| view.id() == run)
    .ok_or(ReviewRefusal::NotWaiting)?;
  let step = return_work::implementing_step(view).ok_or(ReviewRefusal::NoImplementingStep)?;
  let worktree = resolve::read(worktrees, root, (run, false)).map_err(ReviewRefusal::Store)?;
  if let Some(refusal) = return_work::refusal(&detail, view, &worktree) {
    return Err(ReviewRefusal::Returned(refusal));
  }
  let command = return_work::command(view, step, (note, None), Answerer::Person);
  let holding = Holding::hold(unsent.iter().map(|(_, comment)| comment.id()).collect());
  let sending = Said::Comment(CommentState::Sending {
    since: Timestamp::now(),
    attempts: stored.attempts().len(),
  });
  let mut comments = Vec::new();
  for (_, comment) in unsent {
    let changed = db.change_review_message(comment.id(), |said| {
      matches!(said, Said::Comment(CommentState::NotSent { .. })).then(|| sending.clone())
    })?;
    if changed {
      comments.push(comment.id());
    }
  }
  holding.keep(&comments);
  Ok(Sending { run, comments, command })
}

/// Hands `sending` to the runner of `open`, starting it when the project has none, and keeps how that went. Blocks on
/// SQLite once the runner answered. Cut off part-way, the comments are recovered when the run's review is read next.
pub async fn send(runners: &Runners, open: &OpenProject, sending: &Sending) -> Result<(), ReviewRefusal> {
  let _holding = Holding::held(sending.comments.clone());
  let result = runners.control_run_in(open, sending.command.clone()).await;
  sent(&open.repo_root, sending, result, Timestamp::now())
}

/// Keeps how handing `sending` to the runner went, at `now`: taken, its comments are sent and Implement works on them;
/// not taken, every one of them waits unsent again with why, which is the refusal.
pub fn sent(
  root: &Path,
  sending: &Sending,
  result: Result<(), RunRequestError>,
  now: Timestamp,
) -> Result<(), ReviewRefusal> {
  let _holding = Holding::held(sending.comments.clone());
  let mut db = ProjectDb::open(root)?;
  let (after, refusal) = match result {
    Ok(()) => (CommentState::Sent { at: now }, None),
    Err(error) => {
      tracing::info!(%error, run = %sending.run, "the runner did not take the review's comments");
      let failure = failure_of(&error);
      (
        CommentState::NotSent {
          failure: Some(failure.clone()),
        },
        Some(ReviewRefusal::NotSent(failure)),
      )
    }
  };
  for comment in &sending.comments {
    db.change_review_message(*comment, |said| {
      matches!(said, Said::Comment(CommentState::Sending { .. })).then(|| Said::Comment(after.clone()))
    })?;
  }
  refusal.map_or(Ok(()), Err)
}

/// Why a request about a review did not reach Implement, as the thread keeps it.
pub(super) fn failure_of(error: &RunRequestError) -> ReviewFailure {
  match error {
    RunRequestError::Request(RequestError::NotConnected) => ReviewFailure::RunnerUnavailable,
    RunRequestError::Request(RequestError::Lost) => ReviewFailure::Lost,
    RunRequestError::Rejected(rejection) => ReviewFailure::Refused {
      // The rejection's tag is its code; a rejection always serializes, so the debug form never shows.
      reason: serde_json::to_value(rejection)
        .ok()
        .and_then(|value| {
          value
            .get("reason")
            .and_then(|reason| reason.as_str())
            .map(str::to_owned)
        })
        .unwrap_or_else(|| format!("{rejection:?}")),
    },
  }
}
