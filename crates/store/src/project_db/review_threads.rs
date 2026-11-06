//! A person's review of a run's diff (`review_threads`, `review_messages`, `review_viewed`;
//! `orchester_core::review`). Threads and the person's messages are the desktop's; the implementing agent's answer is
//! written by the runner as its turn goes on. Each message's state is a JSON document (`said`) that changes only
//! through [`ProjectDb::change_review_message`], which reads and writes it in one transaction.

use std::num::NonZeroU32;

use orchester_core::{
  conversation::MessageBody,
  id::{ReviewMessageId, ReviewThreadId, RunId},
  pull_request::feedback::{DiffSide, LineAnchor, LineSpan},
  review::{CommentState, QuestionState, ReviewMessage, ReviewThread, Said, ThreadKind, ThreadParts, Viewed},
  revision::Revision,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

const SAID: (&str, u32) = ("review message state", 1);
const THREAD_COLUMNS: &str =
  "id, run_id, kind, path, side, line_start, line_end, excerpt, head, created_at, resolved_at";

impl ProjectDb {
  /// Stores `thread` with its messages.
  pub fn add_review_thread(&mut self, thread: &ReviewThread) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    insert_thread(&transaction, thread)?;
    for (position, message) in (0_i64..).zip(thread.messages()) {
      insert_message(&transaction, thread.id(), position, message)?;
    }
    transaction.commit()?;
    Ok(())
  }

  /// Adds `message` after the others of thread `thread`; the person's reopens the thread when it was resolved, the
  /// agent's leaves it as it is.
  pub fn add_review_message(&mut self, thread: ReviewThreadId, message: &ReviewMessage) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    let next: Option<i64> = transaction.query_row(
      "SELECT MAX(position) + 1 FROM review_messages WHERE thread_id = ?1",
      [thread.to_string()],
      |row| row.get(0),
    )?;
    let next = next.ok_or(StoreError::Unrepresentable(
      "a message of a review thread that is not stored",
    ))?;
    if message.said().is_person() {
      transaction.execute(
        "UPDATE review_threads SET resolved_at = NULL WHERE id = ?1",
        [thread.to_string()],
      )?;
    }
    insert_message(&transaction, thread, next, message)?;
    transaction.commit()?;
    Ok(())
  }

  /// Changes what message `message` says to what `change` makes of it, read and written in one transaction; `false`
  /// when there is no such message or `change` keeps it as it is (`None`).
  pub fn change_review_message(
    &mut self,
    message: ReviewMessageId,
    change: impl FnOnce(&Said) -> Option<Said>,
  ) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    let stored: Option<String> = transaction
      .query_row(
        "SELECT said FROM review_messages WHERE id = ?1",
        [message.to_string()],
        |row| row.get(0),
      )
      .optional()?;
    let Some(stored) = stored else {
      return Ok(false);
    };
    let said: Said = decode_json(SAID.0, SAID.1, &stored)?;
    let Some(changed) = change(&said) else {
      return Ok(false);
    };
    transaction.execute(
      "UPDATE review_messages SET said = ?2 WHERE id = ?1",
      params![message.to_string(), encode_json(SAID.0, SAID.1, &changed)?],
    )?;
    transaction.commit()?;
    Ok(true)
  }

  /// Writes the words of message `message` and what it says now, as the agent's turn goes on.
  pub fn write_review_words(&mut self, message: ReviewMessageId, body: &str, said: &Said) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE review_messages SET body = ?2, said = ?3 WHERE id = ?1",
      params![message.to_string(), body, encode_json(SAID.0, SAID.1, said)?],
    )?;
    Ok(())
  }

  /// Rewrites comment `message` with `body` while it is not sent; `false` once it is (or is not stored).
  pub fn edit_review_comment(&mut self, message: ReviewMessageId, body: &MessageBody) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    if !unsent(&transaction, message)? {
      return Ok(false);
    }
    transaction.execute(
      "UPDATE review_messages SET body = ?2 WHERE id = ?1",
      params![message.to_string(), body.as_str()],
    )?;
    transaction.commit()?;
    Ok(true)
  }

  /// Deletes comment `message` while it is not sent, and its thread when it was the thread's only message; `false`
  /// once it is sent (or is not stored).
  pub fn delete_review_comment(&mut self, message: ReviewMessageId) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    if !unsent(&transaction, message)? {
      return Ok(false);
    }
    let thread: String = transaction.query_row(
      "SELECT thread_id FROM review_messages WHERE id = ?1",
      [message.to_string()],
      |row| row.get(0),
    )?;
    transaction.execute("DELETE FROM review_messages WHERE id = ?1", [message.to_string()])?;
    transaction.execute(
      "DELETE FROM review_threads WHERE id = ?1 AND NOT EXISTS (SELECT 1 FROM review_messages WHERE thread_id = ?1)",
      [&thread],
    )?;
    transaction.commit()?;
    Ok(true)
  }

  /// Resolves thread `thread` at `at`, or reopens it (`None`). `false` when it already stood so.
  pub fn resolve_review_thread(&mut self, thread: ReviewThreadId, at: Option<Timestamp>) -> Result<bool, StoreError> {
    let updated = self.connection.execute(
      "UPDATE review_threads SET resolved_at = ?2 WHERE id = ?1 AND (resolved_at IS NULL) = (?2 IS NOT NULL)",
      params![thread.to_string(), at.map(Timestamp::unix_millis)],
    )?;
    Ok(updated == 1)
  }

  /// `run`'s review threads, oldest first.
  pub fn review_threads(&self, run: RunId) -> Result<Vec<ReviewThread>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "SELECT {THREAD_COLUMNS} FROM review_threads WHERE run_id = ?1 ORDER BY created_at, rowid"
    ))?;
    let stored = statement
      .query_map([run.to_string()], StoredThread::read)?
      .collect::<Result<Vec<_>, _>>()?;
    stored
      .into_iter()
      .map(|thread| load(&self.connection, thread))
      .collect()
  }

  /// Review thread `thread`, if it is stored.
  pub fn review_thread(&self, thread: ReviewThreadId) -> Result<Option<ReviewThread>, StoreError> {
    let stored = self
      .connection
      .query_row(
        &format!("SELECT {THREAD_COLUMNS} FROM review_threads WHERE id = ?1"),
        [thread.to_string()],
        StoredThread::read,
      )
      .optional()?;
    stored.map(|thread| load(&self.connection, thread)).transpose()
  }

  /// The review thread message `message` is in, if it is stored.
  pub fn review_thread_of(&self, message: ReviewMessageId) -> Result<Option<ReviewThread>, StoreError> {
    let thread: Option<String> = self
      .connection
      .query_row(
        "SELECT thread_id FROM review_messages WHERE id = ?1",
        [message.to_string()],
        |row| row.get(0),
      )
      .optional()?;
    match thread {
      Some(thread) => self.review_thread(parse("review thread id", &thread)?),
      None => Ok(None),
    }
  }

  /// Marks file `path` of `run`'s diff at `head` *Viewed* at `at`, or takes the mark off (`None`).
  pub fn mark_review_viewed(
    &mut self,
    (run, head, path): (RunId, &Revision, &str),
    at: Option<Timestamp>,
  ) -> Result<(), StoreError> {
    match at {
      Some(at) => self.connection.execute(
        "INSERT OR IGNORE INTO review_viewed (run_id, head, path, at) VALUES (?1, ?2, ?3, ?4)",
        params![run.to_string(), head.as_str(), path, at.unix_millis()],
      )?,
      None => self.connection.execute(
        "DELETE FROM review_viewed WHERE run_id = ?1 AND head = ?2 AND path = ?3",
        params![run.to_string(), head.as_str(), path],
      )?,
    };
    Ok(())
  }

  /// The files of `run`'s diff marked *Viewed* at `head`, by path.
  pub fn review_viewed(&self, run: RunId, head: &Revision) -> Result<Vec<Viewed>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT path, at FROM review_viewed WHERE run_id = ?1 AND head = ?2 ORDER BY path")?;
    let rows = statement.query_map(params![run.to_string(), head.as_str()], |row| {
      Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    rows
      .map(|row| {
        let (path, at) = row?;
        Ok(Viewed {
          path,
          head: head.clone(),
          at: Timestamp::from_unix_millis(at),
        })
      })
      .collect()
  }

  /// The review questions whose answer is under way, with their runs.
  pub fn answering_review_questions(&self) -> Result<Vec<(RunId, ReviewMessageId)>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT t.run_id, m.id, m.said FROM review_messages m JOIN review_threads t ON t.id = m.thread_id
       WHERE t.kind = 'question'",
    )?;
    let rows = statement
      .query_map([], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, String>(2)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    let mut answering = Vec::new();
    for (run, message, said) in rows {
      let said: Said = decode_json(SAID.0, SAID.1, &said)?;
      if matches!(said, Said::Question(QuestionState::Answering)) {
        answering.push((parse("run id", &run)?, parse("review message id", &message)?));
      }
    }
    Ok(answering)
  }
}

/// Whether `message` is a comment not sent yet.
fn unsent(connection: &Connection, message: ReviewMessageId) -> Result<bool, StoreError> {
  let stored: Option<String> = connection
    .query_row(
      "SELECT said FROM review_messages WHERE id = ?1",
      [message.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  let Some(stored) = stored else {
    return Ok(false);
  };
  let said: Said = decode_json(SAID.0, SAID.1, &stored)?;
  Ok(matches!(said, Said::Comment(CommentState::NotSent { .. })))
}

fn insert_thread(connection: &Connection, thread: &ReviewThread) -> Result<(), StoreError> {
  let anchor = thread.anchor();
  let excerpt = serde_json::to_string(anchor.excerpt()).map_err(|error| corrupt("review excerpt", error))?;
  connection.execute(
    &format!("INSERT INTO review_threads ({THREAD_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"),
    params![
      thread.id().to_string(),
      thread.run().to_string(),
      thread.kind().as_str(),
      anchor.path(),
      anchor.side().as_str(),
      anchor.span().start(),
      anchor.span().end(),
      excerpt,
      thread.head().as_str(),
      thread.created_at().unix_millis(),
      thread.resolved_at().map(Timestamp::unix_millis),
    ],
  )?;
  Ok(())
}

fn insert_message(
  connection: &Connection,
  thread: ReviewThreadId,
  position: i64,
  message: &ReviewMessage,
) -> Result<(), StoreError> {
  connection.execute(
    "INSERT INTO review_messages (id, thread_id, position, said, body, at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    params![
      message.id().to_string(),
      thread.to_string(),
      position,
      encode_json(SAID.0, SAID.1, message.said())?,
      message.body(),
      message.at().unix_millis(),
    ],
  )?;
  Ok(())
}

fn load(connection: &Connection, stored: StoredThread) -> Result<ReviewThread, StoreError> {
  let mut statement =
    connection.prepare("SELECT id, said, body, at FROM review_messages WHERE thread_id = ?1 ORDER BY position")?;
  let messages = statement
    .query_map([&stored.id], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, i64>(3)?,
      ))
    })?
    .collect::<Result<Vec<_>, _>>()?
    .into_iter()
    .map(|(id, said, body, at)| {
      Ok(ReviewMessage::restore(
        parse("review message id", &id)?,
        decode_json(SAID.0, SAID.1, &said)?,
        body,
        Timestamp::from_unix_millis(at),
      ))
    })
    .collect::<Result<Vec<_>, StoreError>>()?;
  ReviewThread::new(stored.into_parts()?, messages).map_err(|error| corrupt("review thread", error))
}

/// One stored thread row, before it is checked.
struct StoredThread {
  id: String,
  run: String,
  kind: String,
  path: String,
  side: String,
  start: i64,
  end: i64,
  excerpt: String,
  head: String,
  created_at: i64,
  resolved_at: Option<i64>,
}

impl StoredThread {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      run: row.get(1)?,
      kind: row.get(2)?,
      path: row.get(3)?,
      side: row.get(4)?,
      start: row.get(5)?,
      end: row.get(6)?,
      excerpt: row.get(7)?,
      head: row.get(8)?,
      created_at: row.get(9)?,
      resolved_at: row.get(10)?,
    })
  }

  fn into_parts(self) -> Result<ThreadParts, StoreError> {
    let side = DiffSide::parse(&self.side).ok_or_else(|| corrupt("review side", &self.side))?;
    let line = |value: i64| {
      u32::try_from(value)
        .ok()
        .and_then(NonZeroU32::new)
        .ok_or_else(|| corrupt("review line", value))
    };
    let span = LineSpan::new(line(self.start)?, line(self.end)?).map_err(|error| corrupt("review lines", error))?;
    let excerpt: Vec<String> = serde_json::from_str(&self.excerpt).map_err(|error| corrupt("review excerpt", error))?;
    let anchor = LineAnchor::new(&self.path, side, span, excerpt).map_err(|error| corrupt("review anchor", error))?;
    Ok(ThreadParts {
      id: parse("review thread id", &self.id)?,
      run: parse("run id", &self.run)?,
      kind: ThreadKind::parse(&self.kind).ok_or_else(|| corrupt("review thread kind", &self.kind))?,
      anchor,
      head: Revision::new(&self.head).map_err(|error| corrupt("review head", error))?,
      created_at: Timestamp::from_unix_millis(self.created_at),
      resolved_at: self.resolved_at.map(Timestamp::from_unix_millis),
    })
  }
}
