//! Agent feedback on pull requests (`feedback_threads`, `feedback_posts`): a person's threads and posts, written
//! by the desktop. A post is sent as a message of the task's conversation in the transaction that stores the
//! message, so a post is never sent twice and never marked sent without its message.

use std::num::NonZeroU32;

use orchester_core::{
  conversation::{Message, MessageBody},
  id::{FeedbackId, FeedbackPostId, IdempotencyKey},
  pull_request::feedback::{
    DiffSide, FeedbackAnchor, FeedbackKind, FeedbackPost, FeedbackThread, LineAnchor, LineSpan, ThreadParts,
  },
  repository::GitHubRepo,
  revision::Revision,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{ProjectDb, messages};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

const THREAD_COLUMNS: &str =
  "id, task_id, number, path, side, line_start, line_end, excerpt, recipient_profile, created_at, resolved_at";
const POST_COLUMNS: &str = "id, kind, body, revision, written_at, message_id, withdrawn_at";

impl ProjectDb {
  /// Stores `thread`, which has its first post only, once per `key`: a retry returns the thread stored the first
  /// time.
  pub fn start_feedback_thread(
    &mut self,
    key: IdempotencyKey,
    thread: &FeedbackThread,
  ) -> Result<FeedbackThread, StoreError> {
    let transaction = self.connection.transaction()?;
    if let Some(existing) = thread_of_key(&transaction, key)? {
      return load_thread(&transaction, thread.repo(), existing);
    }
    insert_thread(&transaction, thread)?;
    insert_post(&transaction, (thread.id(), key), thread.first())?;
    transaction.commit()?;
    Ok(thread.clone())
  }

  /// Adds `post` to thread `thread` once per `key`, reopening the thread in the same transaction when it is resolved,
  /// and returns the thread as stored; a retry adds nothing.
  pub fn add_feedback_post(
    &mut self,
    (key, repo, thread): (IdempotencyKey, &GitHubRepo, FeedbackId),
    post: &FeedbackPost,
  ) -> Result<FeedbackThread, StoreError> {
    let transaction = self.connection.transaction()?;
    if thread_of_key(&transaction, key)?.is_none() {
      transaction.execute(
        "UPDATE feedback_threads SET resolved_at = NULL WHERE id = ?1",
        [thread.to_string()],
      )?;
      let mut stored = load_thread(&transaction, repo, thread)?;
      stored
        .add(post.clone())
        .map_err(|_| StoreError::Unrepresentable("a post in a resolved feedback thread"))?;
      insert_post(&transaction, (thread, key), post)?;
    }
    let stored = load_thread(&transaction, repo, thread)?;
    transaction.commit()?;
    Ok(stored)
  }

  /// Sends held post `post` as `message`, a person's message of the thread's task: the message is stored and the
  /// post marked in one transaction, once. `false` when the post is no longer held: sent or withdrawn meanwhile.
  pub fn send_feedback_post(&mut self, post: FeedbackPostId, message: &Message) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    let held: bool = transaction.query_row(
      "SELECT EXISTS (SELECT 1 FROM feedback_posts WHERE id = ?1 AND message_id IS NULL AND withdrawn_at IS NULL)",
      [post.to_string()],
      |row| row.get(0),
    )?;
    if !held {
      return Ok(false);
    }
    // The post's own id keys the message, so the message is stored once whatever retries it.
    messages::insert(&transaction, message, Some(IdempotencyKey::from_uuid(post.as_uuid())))?;
    transaction.execute(
      "UPDATE feedback_posts SET message_id = ?2 WHERE id = ?1",
      params![post.to_string(), message.id().to_string()],
    )?;
    transaction.commit()?;
    Ok(true)
  }

  /// Withdraws held post `post` at `at`. `false` when it is no longer held.
  pub fn withdraw_feedback_post(&mut self, post: FeedbackPostId, at: Timestamp) -> Result<bool, StoreError> {
    let updated = self.connection.execute(
      "UPDATE feedback_posts SET withdrawn_at = ?2 WHERE id = ?1 AND message_id IS NULL AND withdrawn_at IS NULL",
      params![post.to_string(), at.unix_millis()],
    )?;
    Ok(updated == 1)
  }

  /// Resolves thread `thread` at `at`, or reopens it (`None`). `false` when it already stood so.
  pub fn resolve_feedback_thread(&mut self, thread: FeedbackId, at: Option<Timestamp>) -> Result<bool, StoreError> {
    let updated = self.connection.execute(
      "UPDATE feedback_threads SET resolved_at = ?2 WHERE id = ?1 AND (resolved_at IS NULL) = (?2 IS NOT NULL)",
      params![thread.to_string(), at.map(Timestamp::unix_millis)],
    )?;
    Ok(updated == 1)
  }

  /// The feedback threads on `repo`'s pull request `number`, or on all its pull requests (`None`), oldest first.
  pub fn feedback_threads(&self, repo: &GitHubRepo, number: Option<u64>) -> Result<Vec<FeedbackThread>, StoreError> {
    let number = number
      .map(|number| i64::try_from(number).map_err(|error| corrupt("pull request number", error)))
      .transpose()?;
    let mut statement = self.connection.prepare(
      "SELECT id FROM feedback_threads WHERE host = ?1 AND owner = ?2 AND name = ?3 AND (?4 IS NULL OR number = ?4) \
       ORDER BY created_at, rowid",
    )?;
    let ids = statement
      .query_map(
        params![repo.host().as_str(), repo.owner(), repo.name(), number],
        |row| row.get::<_, String>(0),
      )?
      .collect::<Result<Vec<_>, _>>()?;
    ids
      .iter()
      .map(|id| load_thread(&self.connection, repo, parse("feedback thread id", id)?))
      .collect()
  }
}

/// The thread whose post was stored with `key`.
fn thread_of_key(connection: &Connection, key: IdempotencyKey) -> Result<Option<FeedbackId>, StoreError> {
  let id: Option<String> = connection
    .query_row(
      "SELECT thread_id FROM feedback_posts WHERE idempotency_key = ?1",
      [key.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  id.map(|id| parse("feedback thread id", &id)).transpose()
}

fn insert_thread(connection: &Connection, thread: &FeedbackThread) -> Result<(), StoreError> {
  let lines = thread.anchor().lines();
  let excerpt = lines
    .map(|lines| serde_json::to_string(lines.excerpt()))
    .transpose()
    .map_err(|error| corrupt("feedback excerpt", error))?;
  let repo = thread.repo();
  connection.execute(
    "INSERT INTO feedback_threads (id, task_id, host, owner, name, number, path, side, line_start, line_end, excerpt, \
     recipient_profile, created_at, resolved_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
    params![
      thread.id().to_string(),
      thread.task().to_string(),
      repo.host().as_str(),
      repo.owner(),
      repo.name(),
      i64::try_from(thread.number()).map_err(|error| corrupt("pull request number", error))?,
      lines.map(LineAnchor::path),
      lines.map(|lines| lines.side().as_str()),
      lines.map(|lines| lines.span().start()),
      lines.map(|lines| lines.span().end()),
      excerpt,
      thread.recipient().to_string(),
      thread.created_at().unix_millis(),
      thread.resolved_at().map(Timestamp::unix_millis),
    ],
  )?;
  Ok(())
}

fn insert_post(
  connection: &Connection,
  (thread, key): (FeedbackId, IdempotencyKey),
  post: &FeedbackPost,
) -> Result<(), StoreError> {
  connection.execute(
    "INSERT INTO feedback_posts (id, thread_id, idempotency_key, kind, body, revision, written_at, message_id, \
     withdrawn_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    params![
      post.id().to_string(),
      thread.to_string(),
      key.to_string(),
      post.kind().as_str(),
      post.body().as_str(),
      post.revision().as_str(),
      post.written_at().unix_millis(),
      post.message().map(|message| message.to_string()),
      post.withdrawn_at().map(Timestamp::unix_millis),
    ],
  )?;
  Ok(())
}

fn load_thread(connection: &Connection, repo: &GitHubRepo, id: FeedbackId) -> Result<FeedbackThread, StoreError> {
  let stored = connection.query_row(
    &format!("SELECT {THREAD_COLUMNS} FROM feedback_threads WHERE id = ?1"),
    [id.to_string()],
    StoredThread::read,
  )?;
  let mut statement = connection.prepare(&format!(
    "SELECT {POST_COLUMNS} FROM feedback_posts WHERE thread_id = ?1 ORDER BY written_at, rowid"
  ))?;
  let posts = statement
    .query_map([id.to_string()], StoredPost::read)?
    .collect::<Result<Vec<_>, _>>()?
    .into_iter()
    .map(StoredPost::into_post)
    .collect::<Result<Vec<_>, _>>()?;
  FeedbackThread::new(stored.into_parts(repo)?, posts).map_err(|error| corrupt("feedback thread", error))
}

/// One stored thread row, before it is checked.
struct StoredThread {
  id: String,
  task: String,
  number: i64,
  path: Option<String>,
  side: Option<String>,
  start: Option<i64>,
  end: Option<i64>,
  excerpt: Option<String>,
  recipient: String,
  created_at: i64,
  resolved_at: Option<i64>,
}

impl StoredThread {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      task: row.get(1)?,
      number: row.get(2)?,
      path: row.get(3)?,
      side: row.get(4)?,
      start: row.get(5)?,
      end: row.get(6)?,
      excerpt: row.get(7)?,
      recipient: row.get(8)?,
      created_at: row.get(9)?,
      resolved_at: row.get(10)?,
    })
  }

  fn into_parts(self, repo: &GitHubRepo) -> Result<ThreadParts, StoreError> {
    let anchor = match (self.path, self.side, self.start, self.end, self.excerpt) {
      (None, None, None, None, None) => FeedbackAnchor::PullRequest,
      (Some(path), Some(side), Some(start), Some(end), Some(excerpt)) => {
        let side = DiffSide::parse(&side).ok_or_else(|| corrupt("feedback side", &side))?;
        let line = |value: i64| {
          u32::try_from(value)
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or_else(|| corrupt("feedback line", value))
        };
        let span = LineSpan::new(line(start)?, line(end)?).map_err(|error| corrupt("feedback lines", error))?;
        let excerpt: Vec<String> =
          serde_json::from_str(&excerpt).map_err(|error| corrupt("feedback excerpt", error))?;
        FeedbackAnchor::Lines(
          LineAnchor::new(&path, side, span, excerpt).map_err(|error| corrupt("feedback anchor", error))?,
        )
      }
      _ => return Err(corrupt("feedback anchor", "partly stored")),
    };
    Ok(ThreadParts {
      id: parse("feedback thread id", &self.id)?,
      task: parse("task id", &self.task)?,
      repo: repo.clone(),
      number: u64::try_from(self.number).map_err(|error| corrupt("pull request number", error))?,
      anchor,
      recipient: parse("agent profile id", &self.recipient)?,
      created_at: Timestamp::from_unix_millis(self.created_at),
      resolved_at: self.resolved_at.map(Timestamp::from_unix_millis),
    })
  }
}

/// One stored post row, before it is checked.
struct StoredPost {
  id: String,
  kind: String,
  body: String,
  revision: String,
  written_at: i64,
  message: Option<String>,
  withdrawn_at: Option<i64>,
}

impl StoredPost {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      kind: row.get(1)?,
      body: row.get(2)?,
      revision: row.get(3)?,
      written_at: row.get(4)?,
      message: row.get(5)?,
      withdrawn_at: row.get(6)?,
    })
  }

  fn into_post(self) -> Result<FeedbackPost, StoreError> {
    let kind = FeedbackKind::parse(&self.kind).ok_or_else(|| corrupt("feedback kind", &self.kind))?;
    let body = MessageBody::new(&self.body).map_err(|error| corrupt("feedback post", error))?;
    let revision = Revision::new(&self.revision).map_err(|error| corrupt("feedback revision", error))?;
    let message = self
      .message
      .as_deref()
      .map(|message| parse("message id", message))
      .transpose()?;
    FeedbackPost::restore(
      (parse("feedback post id", &self.id)?, kind, body),
      (revision, Timestamp::from_unix_millis(self.written_at)),
      (message, self.withdrawn_at.map(Timestamp::from_unix_millis)),
    )
    .map_err(|error| corrupt("feedback post", error))
  }
}
