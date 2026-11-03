//! Side questions about a run (`side_questions`, ORC-337): the desktop keeps a question as it is asked, then how it
//! ended, once; an answer is marked sent on to the run's agent once. They belong to their task and never join its
//! conversation.

use orchester_core::{
  id::{SideQuestionId, TaskId},
  run::side_question::{SideFailure, SideQuestion, SideQuestionText, SideReply, SideState},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Row, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

/// The schema of a stored answer and of a stored failure.
const ANSWER_SCHEMA: u32 = 1;
const FAILURE_SCHEMA: u32 = 1;
const COLUMNS: &str = "id, task_id, run_id, question, asked_at, state, answer, model, failure, ended_at, sent_at";

impl ProjectDb {
  /// Keeps `question`, just asked and being answered.
  pub fn keep_side_question(&mut self, question: &SideQuestion) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO side_questions (id, task_id, run_id, question, asked_at, state) \
       VALUES (?1, ?2, ?3, ?4, ?5, 'answering')",
      params![
        question.id.to_string(),
        question.task.to_string(),
        question.run.to_string(),
        question.question.as_str(),
        question.asked_at.unix_millis(),
      ],
    )?;
    Ok(())
  }

  /// Ends side question `id`, still being answered, with `ended` at `at`: the answer and the model that gave it, or
  /// why there is none. Whether it was being answered: a question that ended already keeps how.
  pub fn end_side_question(
    &mut self,
    id: SideQuestionId,
    ended: Result<(&SideReply, &str), &SideFailure>,
    at: Timestamp,
  ) -> Result<bool, StoreError> {
    let changed = match ended {
      Ok((reply, model)) => self.connection.execute(
        "UPDATE side_questions SET state = 'answered', answer = ?2, model = ?3, ended_at = ?4 \
         WHERE id = ?1 AND state = 'answering'",
        params![
          id.to_string(),
          encode_json("side answer", ANSWER_SCHEMA, reply)?,
          model,
          at.unix_millis()
        ],
      )?,
      Err(failure) => self.connection.execute(
        "UPDATE side_questions SET state = 'failed', failure = ?2, ended_at = ?3 WHERE id = ?1 AND state = 'answering'",
        params![
          id.to_string(),
          encode_json("side failure", FAILURE_SCHEMA, failure)?,
          at.unix_millis()
        ],
      )?,
    };
    Ok(changed == 1)
  }

  /// Marks the answer of side question `id` sent on to the run's agent at `at`; whether it was answered and not sent
  /// yet.
  pub fn mark_side_question_sent(&mut self, id: SideQuestionId, at: Timestamp) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "UPDATE side_questions SET sent_at = ?2 WHERE id = ?1 AND state = 'answered' AND sent_at IS NULL",
      params![id.to_string(), at.unix_millis()],
    )?;
    Ok(changed == 1)
  }

  /// The side questions of `task`, in the order they were asked.
  pub fn side_questions(&self, task: TaskId) -> Result<Vec<SideQuestion>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "SELECT {COLUMNS} FROM side_questions WHERE task_id = ?1 ORDER BY asked_at, rowid"
    ))?;
    let rows = statement
      .query_map([task.to_string()], stored)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(StoredQuestion::into_question).collect()
  }

  /// Side question `id`, if it is kept.
  pub fn side_question(&self, id: SideQuestionId) -> Result<Option<SideQuestion>, StoreError> {
    let row = self
      .connection
      .query_row(
        &format!("SELECT {COLUMNS} FROM side_questions WHERE id = ?1"),
        [id.to_string()],
        stored,
      )
      .optional()?;
    row.map(StoredQuestion::into_question).transpose()
  }
}

/// A `side_questions` row as stored.
struct StoredQuestion {
  id: String,
  task: String,
  run: String,
  question: String,
  asked_at: i64,
  state: String,
  answer: Option<String>,
  model: Option<String>,
  failure: Option<String>,
  ended_at: Option<i64>,
  sent_at: Option<i64>,
}

fn stored(row: &Row<'_>) -> rusqlite::Result<StoredQuestion> {
  Ok(StoredQuestion {
    id: row.get(0)?,
    task: row.get(1)?,
    run: row.get(2)?,
    question: row.get(3)?,
    asked_at: row.get(4)?,
    state: row.get(5)?,
    answer: row.get(6)?,
    model: row.get(7)?,
    failure: row.get(8)?,
    ended_at: row.get(9)?,
    sent_at: row.get(10)?,
  })
}

impl StoredQuestion {
  fn into_question(self) -> Result<SideQuestion, StoreError> {
    let ended = || {
      self
        .ended_at
        .map(Timestamp::from_unix_millis)
        .ok_or_else(|| corrupt("side question end", "an ended question has no end time"))
    };
    let state = match (self.state.as_str(), &self.answer, &self.model, &self.failure) {
      ("answering", ..) => SideState::Answering,
      ("answered", Some(answer), Some(model), _) => SideState::Answered {
        reply: decode_json("side answer", ANSWER_SCHEMA, answer)?,
        model: model.clone(),
        at: ended()?,
        sent: self.sent_at.map(Timestamp::from_unix_millis),
      },
      ("failed", _, _, Some(failure)) => SideState::Failed {
        failure: decode_json::<SideFailure>("side failure", FAILURE_SCHEMA, failure)?,
        at: ended()?,
      },
      (other, ..) => return Err(corrupt("side question state", other)),
    };
    Ok(SideQuestion {
      id: parse("side question id", &self.id)?,
      task: parse("task id", &self.task)?,
      run: parse("run id", &self.run)?,
      question: SideQuestionText::new(&self.question).map_err(|error| corrupt("side question", error))?,
      asked_at: Timestamp::from_unix_millis(self.asked_at),
      state,
    })
  }
}
