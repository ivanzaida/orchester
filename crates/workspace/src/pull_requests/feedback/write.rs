//! What a person does with feedback threads: starts one on lines of the diff or on the whole pull request, replies or
//! asks for a fix in one, withdraws a request Orchester still holds, sends again a request its agent's CLI did not
//! take, resolves a thread and reopens it. Every write is checked against the pull request and its task as stored
//! now, never against what a screen last showed. Each blocks on SQLite.

use std::path::Path;

use orchester_core::{
  conversation::{MessageBody, Recipient},
  id::{FeedbackId, FeedbackPostId, IdempotencyKey, MessageId, ResourceId, TaskId},
  pull_request::feedback::{FeedbackAnchor, FeedbackKind, FeedbackPost, FeedbackThread, ThreadParts},
  revision::Revision,
  store::{PullRequestDiffStore, PullRequestStore, RepositoryStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::FeedbackError;
use crate::{
  pull_requests::linked::linked_tasks,
  tasks::{
    conversation::{SendError, composer::ComposerMode, outgoing::conversation_of, send_after_turn_in},
    detail::TaskDetail,
  },
};

/// A new thread as the person wrote it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewThread {
  pub number: u64,
  pub anchor: FeedbackAnchor,
  pub kind: FeedbackKind,
  pub text: String,
  /// The pull request's head the person wrote it at.
  pub revision: Revision,
  pub recipient: ResourceId,
}

/// Starts `new` once per `key`; a retry returns the thread stored the first time. Refused when its recipient is not
/// one of the task's agents or the task takes no more messages ([`takes_requests`]).
pub fn start_in(root: &Path, key: IdempotencyKey, new: &NewThread) -> Result<FeedbackThread, FeedbackError> {
  let mut db = ProjectDb::open(root)?;
  let (repo, task) = linked(&db, new.number, &new.revision)?;
  let body = MessageBody::new(&new.text).map_err(|_| FeedbackError::Blank)?;
  takes_requests(root, task, new.recipient)?;
  let now = Timestamp::now();
  let thread = FeedbackThread::new(
    ThreadParts {
      id: FeedbackId::generate(),
      task,
      repo,
      number: new.number,
      anchor: new.anchor.clone(),
      recipient: new.recipient,
      created_at: now,
      resolved_at: None,
    },
    vec![FeedbackPost::new(new.kind, body, new.revision.clone(), now)],
  )?;
  Ok(db.start_feedback_thread(key, &thread)?)
}

/// Adds the person's `text` to thread `thread` of pull request `number` once per `key`, asking `kind` of the agent at
/// `revision`, the head they wrote it at; a resolved thread is reopened in the same write.
pub fn reply_in(
  root: &Path,
  key: IdempotencyKey,
  (number, thread): (u64, FeedbackId),
  (kind, text, revision): (FeedbackKind, &str, &Revision),
) -> Result<FeedbackThread, FeedbackError> {
  let mut db = ProjectDb::open(root)?;
  let (repo, _) = linked(&db, number, revision)?;
  let body = MessageBody::new(text).map_err(|_| FeedbackError::Blank)?;
  thread_of(&db, (&repo, number), thread)?;
  let post = FeedbackPost::new(kind, body, revision.clone(), Timestamp::now());
  Ok(db.add_feedback_post((key, &repo, thread), &post)?)
}

/// Withdraws post `post` of thread `thread`, held by Orchester; refused with [`FeedbackError::AlreadySent`] when it
/// went to the agent meanwhile.
pub fn withdraw_in(
  root: &Path,
  (number, thread): (u64, FeedbackId),
  post: FeedbackPostId,
) -> Result<(), FeedbackError> {
  let mut db = ProjectDb::open(root)?;
  let repo = repo_of(&db)?;
  if thread_of(&db, (&repo, number), thread)?.post(post).is_none() {
    return Err(FeedbackError::NotFound);
  }
  if db.withdraw_feedback_post(post, Timestamp::now())? {
    Ok(())
  } else {
    Err(FeedbackError::AlreadySent)
  }
}

/// Resolves thread `thread` (`resolved`) or reopens it; `false` when it already stood so. It closes the thread in
/// Orchester only: the pull request is not approved or merged.
pub fn resolve_in(root: &Path, (number, thread): (u64, FeedbackId), resolved: bool) -> Result<bool, FeedbackError> {
  let mut db = ProjectDb::open(root)?;
  let repo = repo_of(&db)?;
  thread_of(&db, (&repo, number), thread)?;
  let at = resolved.then(Timestamp::now);
  Ok(db.resolve_feedback_thread(thread, at)?)
}

/// Sends message `message` of thread `thread` again after its agent's turn: its CLI did not take it.
pub fn retry_in(root: &Path, (number, thread): (u64, FeedbackId), message: MessageId) -> Result<(), FeedbackError> {
  let db = ProjectDb::open(root)?;
  let repo = repo_of(&db)?;
  let stored = thread_of(&db, (&repo, number), thread)?;
  if !stored.posts().iter().any(|post| post.message() == Some(message)) {
    return Err(FeedbackError::NotFound);
  }
  drop(db);
  send_after_turn_in(root, stored.task(), message).map_err(FeedbackError::Send)?;
  Ok(())
}

fn repo_of(db: &ProjectDb) -> Result<orchester_core::repository::GitHubRepo, FeedbackError> {
  Ok(db.repository_link()?.ok_or(FeedbackError::Unlinked)?.repo().clone())
}

/// Refuses a new thread for agent `recipient` of `task` when it is not one of the task's agents, or when the task's
/// conversation takes no more messages: what the board's gate says of it ([`super::Gate::Closed`]), the runner taken
/// as serving the project, so a request its return would let go is kept.
fn takes_requests(root: &Path, task: TaskId, recipient: ResourceId) -> Result<(), FeedbackError> {
  let detail = TaskDetail::load_in(root, task)?;
  let conversation = conversation_of(&detail, detail.lost_its_runner(true));
  let agent = conversation
    .recipients
    .iter()
    .any(|choice| matches!(choice.recipient, Recipient::Agent(profile) if profile == recipient));
  if !agent {
    return Err(FeedbackError::NotAnAgent);
  }
  match ComposerMode::of(&conversation, detail.run.as_ref(), Recipient::Agent(recipient)) {
    ComposerMode::Closed | ComposerMode::RunAgain => Err(FeedbackError::Closed),
    _ => Ok(()),
  }
}

/// The thread `thread` of pull request `number`.
fn thread_of(
  db: &ProjectDb,
  (repo, number): (&orchester_core::repository::GitHubRepo, u64),
  thread: FeedbackId,
) -> Result<FeedbackThread, FeedbackError> {
  db.feedback_threads(repo, Some(number))?
    .into_iter()
    .find(|stored| stored.id() == thread)
    .ok_or(FeedbackError::NotFound)
}

/// The linked repository and the task of its pull request `number`, which the person saw at `seen`: refused when
/// the pull request has no task or its head moved since ([`known_head`]).
fn linked(
  db: &ProjectDb,
  number: u64,
  seen: &Revision,
) -> Result<(orchester_core::repository::GitHubRepo, orchester_core::id::TaskId), FeedbackError> {
  let repo = repo_of(db)?;
  let synced = db.pull_requests(&repo)?.ok_or(FeedbackError::NotFound)?;
  let pull_request = synced
    .pull_requests
    .iter()
    .find(|pull_request| pull_request.number() == number)
    .ok_or(FeedbackError::NotFound)?;
  let head = known_head(db, &repo, (pull_request, synced.synced_at))?;
  if head != *seen {
    return Err(FeedbackError::HeadMoved(head));
  }
  let task = linked_tasks(db, std::slice::from_ref(pull_request))?
    .remove(&number)
    .ok_or(FeedbackError::NoTask)?;
  Ok((repo, task.task))
}

/// The pull request's latest head Orchester read: the head of the diff its page read, when that was after the list's
/// sync (`synced_at`) stored `pull_request`; else the one the sync stored.
pub(super) fn known_head(
  db: &ProjectDb,
  repo: &orchester_core::repository::GitHubRepo,
  (pull_request, synced_at): (&orchester_core::pull_request::PullRequest, Timestamp),
) -> Result<Revision, FeedbackError> {
  let diff = db.latest_pull_request_diff(repo, pull_request.number())?;
  Ok(match diff {
    Some(diff) if diff.read_at >= synced_at => diff.head,
    _ => pull_request.head_sha().clone(),
  })
}

impl From<SendError> for FeedbackError {
  fn from(error: SendError) -> Self {
    Self::Send(error)
  }
}
