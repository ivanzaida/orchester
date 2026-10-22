//! Agent feedback on a pull request (`docs/design/orchester-workspace.md`, "Pull request review and agent
//! feedback"; ORC-14): a person's questions and fix requests on lines of the diff or on the whole pull request, for
//! the agent of the pull request's linked task, kept in Orchester and never posted to GitHub. A request goes to the
//! agent as a message of the task's conversation once the run's state lets it ([`dispatch`]); where each thread
//! stands comes from that conversation and the run ([`board`]). Every function blocks on SQLite (and git for a fix's
//! commit); the UI calls them off the UI thread.

mod board;
mod dispatch;
mod sources;
mod state;
mod write;

use orchester_core::{
  pull_request::feedback::FeedbackError as Invalid,
  revision::Revision,
  store::{PullRequestStore, RepositoryStore},
};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;

pub use self::{
  board::{
    BoardTask, Budget, CommitView, FeedbackBoard, Gate, Inputs, PostView, ReplyView, RunFacts, Target, ThreadView,
    WorkingView,
  },
  dispatch::{Dispatched, Slots, dispatch_in, tell_runner},
  sources::SourceRef,
  state::{Hold, NotDelivered, ThreadEvent, ThreadState},
  write::{NewThread, reply_in, resolve_in, retry_in, start_in, withdraw_in},
};
use super::linked::linked_tasks;
use crate::{
  messages::{self, Message},
  tasks::{
    conversation::{SendError, outgoing::conversation_of},
    detail::TaskDetail,
  },
};

/// Why feedback could not be read or written.
#[derive(Debug, Error)]
pub enum FeedbackError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Invalid(#[from] Invalid),
  #[error("the project has no linked GitHub repository")]
  Unlinked,
  #[error("the pull request or the thread is not stored")]
  NotFound,
  #[error("the pull request has no linked task")]
  NoTask,
  #[error("the pull request's head moved to {0}")]
  HeadMoved(Revision),
  #[error("a request says something")]
  Blank,
  #[error("the message was not sent again")]
  Send(SendError),
}

impl FeedbackError {
  pub fn message(&self) -> Message {
    match self {
      Self::Store(error) => messages::store_error(error),
      Self::Invalid(_) => Message::new("pullRequests.feedback.errors.invalid"),
      Self::Unlinked => Message::new("pullRequests.feedback.errors.unlinked"),
      Self::NotFound => Message::new("pullRequests.feedback.errors.notFound"),
      Self::NoTask => Message::new("pullRequests.feedback.errors.noTask"),
      Self::HeadMoved(revision) => Message::new("pullRequests.feedback.errors.headMoved")
        .with("revision", crate::display::short_revision(revision.as_str()).to_owned()),
      Self::Blank => Message::new("pullRequests.feedback.errors.blank"),
      Self::Send(error) => error.message(),
    }
  }
}

/// The agent feedback of pull request `number` of the project at `inputs.root`.
pub fn load_board_in(inputs: &Inputs<'_>, number: u64) -> Result<FeedbackBoard, FeedbackError> {
  let db = ProjectDb::open(inputs.root)?;
  let link = db.repository_link()?.ok_or(FeedbackError::Unlinked)?;
  let threads = db.feedback_threads(link.repo(), Some(number))?;
  let pull_request = db.pull_requests(link.repo())?.and_then(|synced| {
    synced
      .pull_requests
      .into_iter()
      .find(|pull_request| pull_request.number() == number)
  });
  let task = match pull_request {
    Some(pull_request) => linked_tasks(&db, std::slice::from_ref(&pull_request))?.remove(&number),
    None => None,
  };
  let Some(task) = task else {
    return Ok(board::board(inputs, None, &threads));
  };
  let detail = TaskDetail::load_in(inputs.root, task.task)?;
  let checkpoints = match &detail.run {
    Some(run) => db.run_checkpoints(run.id())?,
    None => Vec::new(),
  };
  drop(db);
  let conversation = conversation_of(&detail, detail.lost_its_runner(!inputs.lost));
  Ok(board::board(
    inputs,
    Some((&detail, &conversation, &checkpoints)),
    &threads,
  ))
}
