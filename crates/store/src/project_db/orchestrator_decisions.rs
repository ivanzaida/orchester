//! The decisions the Orchestrator made on its own (`orchester_core::orchestrator::decision`): kept when the call that
//! made one was done, read for the person and every client, and reviewed by the person only.

use orchester_core::{
  id::ChatId,
  orchestrator::decision::{Decision, NewDecision, Reason, Review, Undo},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Row, params};

use super::ProjectDb;
use crate::{StoreError, rows::parse};

/// Which decisions a reading wants: every one when nothing is set, the open ones first and then the newest first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecisionsRead {
  /// Only those the person has not looked at.
  pub open_only: bool,
  /// Only those on the task with this key, in any case.
  pub task: Option<String>,
  /// Only those of this conversation.
  pub chat: Option<ChatId>,
  /// Only those made at or after this time.
  pub since: Option<Timestamp>,
  /// Only those made before this time.
  pub until: Option<Timestamp>,
}

const COLUMNS: &str = "id, chat_id, tool, task_key, run_id, reason, undo, at, review, reviewed_at";

impl ProjectDb {
  /// Keeps `decision`, open; its number in the project.
  pub fn keep_decision(&mut self, decision: &NewDecision) -> Result<i64, StoreError> {
    self.connection.execute(
      "INSERT INTO orchestrator_decisions (chat_id, tool, task_key, run_id, reason, undo, at)
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
      params![
        decision.chat.to_string(),
        decision.tool,
        decision.task,
        decision.run.map(|run| run.to_string()),
        decision.reason.as_str(),
        decision.undo.map(Undo::name),
        decision.at.unix_millis(),
      ],
    )?;
    Ok(self.connection.last_insert_rowid())
  }

  /// The decisions `read` wants: the open ones first, then the newest first.
  pub fn decisions(&self, read: &DecisionsRead) -> Result<Vec<Decision>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "SELECT {COLUMNS} FROM orchestrator_decisions
       WHERE (?1 IS NULL OR task_key = ?1 COLLATE NOCASE)
         AND (?2 IS NULL OR chat_id = ?2)
         AND (?3 IS NULL OR at >= ?3)
         AND (?4 IS NULL OR at < ?4)
         AND (?5 = 0 OR review = 'open')
       ORDER BY (review = 'open') DESC, at DESC, id DESC"
    ))?;
    let rows = statement
      .query_map(
        params![
          read.task,
          read.chat.map(|chat| chat.to_string()),
          read.since.map(Timestamp::unix_millis),
          read.until.map(Timestamp::unix_millis),
          read.open_only,
        ],
        stored,
      )?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(Stored::decision).collect()
  }

  /// Reviews decision `number` as `review`, when its review may become that
  /// ([`Review::may_become`]); whether it did. A number the project does not keep changes nothing.
  pub fn review_decision(&mut self, number: i64, review: Review) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    let now = transaction
      .query_row(
        &format!("SELECT {COLUMNS} FROM orchestrator_decisions WHERE id = ?1"),
        [number],
        stored,
      )
      .optional()?;
    let Some(now) = now else {
      return Ok(false);
    };
    if !now.decision()?.review.may_become(review) {
      return Ok(false);
    }
    let at = match review {
      Review::Open => None,
      Review::Seen { at } | Review::Overruled { at } => Some(at.unix_millis()),
    };
    transaction.execute(
      "UPDATE orchestrator_decisions SET review = ?2, reviewed_at = ?3 WHERE id = ?1",
      params![number, review.name(), at],
    )?;
    transaction.commit()?;
    Ok(true)
  }
}

/// A row as it is stored, before its ids and names are read.
struct Stored {
  number: i64,
  chat: String,
  tool: String,
  task: Option<String>,
  run: Option<String>,
  reason: String,
  undo: Option<String>,
  at: i64,
  review: String,
  reviewed_at: Option<i64>,
}

fn stored(row: &Row<'_>) -> rusqlite::Result<Stored> {
  Ok(Stored {
    number: row.get(0)?,
    chat: row.get(1)?,
    tool: row.get(2)?,
    task: row.get(3)?,
    run: row.get(4)?,
    reason: row.get(5)?,
    undo: row.get(6)?,
    at: row.get(7)?,
    review: row.get(8)?,
    reviewed_at: row.get(9)?,
  })
}

impl Stored {
  fn decision(self) -> Result<Decision, StoreError> {
    let reviewed = self.reviewed_at.map(Timestamp::from_unix_millis);
    let review = match (self.review.as_str(), reviewed) {
      ("seen", Some(at)) => Review::Seen { at },
      ("overruled", Some(at)) => Review::Overruled { at },
      // A state this version does not know reads as not looked at: the person still sees the decision.
      _ => Review::Open,
    };
    let reason = Reason::new(&self.reason).ok_or_else(|| StoreError::Corrupt {
      what: "a decision's reason",
      detail: self.reason.clone(),
    })?;
    Ok(Decision {
      number: self.number,
      made: NewDecision {
        chat: parse("chat id", &self.chat)?,
        tool: self.tool,
        task: self.task,
        run: self.run.as_deref().map(|run| parse("run id", run)).transpose()?,
        reason,
        // An undo this version does not know promises nothing.
        undo: self.undo.as_deref().and_then(Undo::named),
        at: Timestamp::from_unix_millis(self.at),
      },
      review,
    })
  }
}
