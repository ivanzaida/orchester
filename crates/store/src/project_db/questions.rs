//! Agents' questions with suggested answers (`orchester_core::question`), written by the runner: asked,
//! answered once, or interrupted when the asking agent can no longer take an answer.

use orchester_core::{
  id::{InvocationId, MessageId, RunId},
  orchestrator::FromOrchestrator,
  question::{Answer, AskedQuestions, GivenAnswer, Question, QuestionRequest, QuestionState, QuestionTurn},
  run::Answerer,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::{ProjectDb, orchestrator_sent::OrchestratorColumns};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// The columns [`StoredQuestions::read`] reads, in its order.
const COLUMNS: &str = "id, run_id, step_execution_id, invocation_id, agent, questions, turn, asked_at, state, answers, \
                       answered_by, ended_at, message_id, by_orchestrator, orchestrator_chat, orchestrator_way";

impl ProjectDb {
  /// Records request `request`, waiting for its answer. A repeated id in the run is refused.
  pub fn ask_questions(&mut self, request: &QuestionRequest) -> Result<(), StoreError> {
    let questions = serde_json::to_string(request.questions()).map_err(|_| StoreError::Unrepresentable("questions"))?;
    self.connection.execute(
      "INSERT INTO questions (id, run_id, step_execution_id, invocation_id, agent, questions, turn, asked_at) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
      params![
        request.id(),
        request.run().to_string(),
        request.execution().to_string(),
        request.invocation().to_string(),
        request.agent(),
        questions,
        request.turn().as_str(),
        request.asked_at().unix_millis(),
      ],
    )?;
    Ok(())
  }

  /// `run`'s requests, oldest first, with where each stands.
  pub fn questions(&self, run: RunId) -> Result<Vec<AskedQuestions>, StoreError> {
    self.questions_where("run_id = ?1", Some(run))
  }

  /// The project's requests that wait for an answer, oldest first across runs.
  pub fn waiting_questions(&self) -> Result<Vec<AskedQuestions>, StoreError> {
    self.questions_where("state = 'waiting' AND ?1 IS NULL", None)
  }

  fn questions_where(&self, filter: &str, run: Option<RunId>) -> Result<Vec<AskedQuestions>, StoreError> {
    let mut query = self.connection.prepare(&format!(
      "SELECT {COLUMNS} FROM questions WHERE {filter} ORDER BY asked_at, id"
    ))?;
    let rows = query
      .query_map([run.map(|run| run.to_string())], StoredQuestions::read)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(StoredQuestions::into_asked).collect()
  }

  /// Request `id` of `run`, if there is one.
  pub fn question_request(&self, run: RunId, id: &str) -> Result<Option<AskedQuestions>, StoreError> {
    let row = self
      .connection
      .query_row(
        &format!("SELECT {COLUMNS} FROM questions WHERE run_id = ?1 AND id = ?2"),
        params![run.to_string(), id],
        StoredQuestions::read,
      )
      .optional()?;
    row.map(StoredQuestions::into_asked).transpose()
  }

  /// Records `answers` to waiting request `id` of `run`, given `by` at `at`, and the person's message
  /// they reach a step-end question's agent in; `false` when it no longer waits (answered or interrupted).
  pub fn answer_questions(
    &mut self,
    (run, id): (RunId, &str),
    answers: &[Answer],
    (by, at): (Answerer, Timestamp),
    message: Option<MessageId>,
  ) -> Result<bool, StoreError> {
    let stored: Vec<GivenAnswer> = answers.iter().map(GivenAnswer::from).collect();
    let stored = serde_json::to_string(&stored).map_err(|_| StoreError::Unrepresentable("question answers"))?;
    let changed = self.connection.execute(
      "UPDATE questions SET state = 'answered', answers = ?3, answered_by = ?4, ended_at = ?5, message_id = ?6 \
       WHERE run_id = ?1 AND id = ?2 AND state = 'waiting'",
      params![
        run.to_string(),
        id,
        stored,
        answerer(by),
        at.unix_millis(),
        message.map(|message| message.to_string()),
      ],
    )?;
    Ok(changed == 1)
  }

  /// The agent of `invocation` in `run` ended: its requests that still wait in its turn can no longer be
  /// answered. Whether any did.
  pub fn interrupt_questions(&mut self, run: RunId, invocation: InvocationId) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "UPDATE questions SET state = 'interrupted', ended_at = ?3 \
       WHERE run_id = ?1 AND invocation_id = ?2 AND state = 'waiting' AND turn = 'in_turn'",
      params![run.to_string(), invocation.to_string(), Timestamp::now().unix_millis()],
    )?;
    Ok(changed > 0)
  }

  /// Request `id` of `run` waits for an agent that can no longer take an answer: it ends interrupted.
  /// Whether it still waited.
  pub fn end_waiting_question(&mut self, run: RunId, id: &str) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "UPDATE questions SET state = 'interrupted', ended_at = ?3 WHERE run_id = ?1 AND id = ?2 AND state = 'waiting'",
      params![run.to_string(), id, Timestamp::now().unix_millis()],
    )?;
    Ok(changed == 1)
  }

  /// Request `id` of `run`, answered, never reached its agent: it ends interrupted, answered by no one.
  pub fn undelivered_questions(&mut self, run: RunId, id: &str) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE questions SET state = 'interrupted', answers = NULL, answered_by = NULL, message_id = NULL, \
       by_orchestrator = 0, orchestrator_chat = NULL, orchestrator_way = NULL \
       WHERE run_id = ?1 AND id = ?2 AND state = 'answered'",
      params![run.to_string(), id],
    )?;
    Ok(())
  }

  /// The Orchestrator chat answered request `id` of `run`, as automation, from the conversation and in the way `from`
  /// says, and the runner recorded it: its card names the chat. Only an answer automation gave is marked; whether it
  /// was marked.
  pub fn mark_answered_by_orchestrator(
    &mut self,
    (run, id): (RunId, &str),
    from: FromOrchestrator,
  ) -> Result<bool, StoreError> {
    let columns = OrchestratorColumns::of(Some(from));
    let changed = self.connection.execute(
      "UPDATE questions SET by_orchestrator = 1, orchestrator_chat = ?3, orchestrator_way = ?4 \
       WHERE run_id = ?1 AND id = ?2 AND state = 'answered' AND answered_by = 'automation'",
      params![run.to_string(), id, columns.chat, columns.way],
    )?;
    Ok(changed == 1)
  }
}

/// Who answered or decided, as `questions.answered_by` and `plans.decided_by` keep it.
pub(super) const fn answerer(by: Answerer) -> &'static str {
  match by {
    Answerer::Person => "person",
    Answerer::Automation => "automation",
  }
}

/// [`answerer`]'s value `stored` read back; `what` names the column for a value it never writes.
pub(super) fn read_answerer(what: &'static str, stored: Option<&str>) -> Result<Answerer, StoreError> {
  match stored {
    Some("person") => Ok(Answerer::Person),
    Some("automation") => Ok(Answerer::Automation),
    other => Err(corrupt(what, format!("{other:?}"))),
  }
}

/// A `questions` row as read, before its ids, questions and answers are checked.
struct StoredQuestions {
  id: String,
  run: String,
  execution: String,
  invocation: String,
  agent: String,
  questions: String,
  turn: String,
  asked_at: i64,
  state: String,
  answers: Option<String>,
  answered_by: Option<String>,
  ended_at: Option<i64>,
  message: Option<String>,
  orchestrator: OrchestratorColumns,
}

impl StoredQuestions {
  fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      run: row.get(1)?,
      execution: row.get(2)?,
      invocation: row.get(3)?,
      agent: row.get(4)?,
      questions: row.get(5)?,
      turn: row.get(6)?,
      asked_at: row.get(7)?,
      state: row.get(8)?,
      answers: row.get(9)?,
      answered_by: row.get(10)?,
      ended_at: row.get(11)?,
      message: row.get(12)?,
      orchestrator: OrchestratorColumns::read(row, 13)?,
    })
  }

  fn into_asked(self) -> Result<AskedQuestions, StoreError> {
    let questions: Vec<Question> =
      serde_json::from_str(&self.questions).map_err(|error| corrupt("questions", error))?;
    let turn = QuestionTurn::parse(&self.turn).ok_or_else(|| corrupt("question turn", &self.turn))?;
    let request = QuestionRequest::new(
      self.id,
      (
        parse("question run id", &self.run)?,
        parse("question execution id", &self.execution)?,
        parse("question invocation id", &self.invocation)?,
      ),
      self.agent,
      questions,
      (Timestamp::from_unix_millis(self.asked_at), turn),
    )
    .map_err(|error| corrupt("questions", error))?;
    let ended = || {
      self
        .ended_at
        .map(Timestamp::from_unix_millis)
        .ok_or_else(|| corrupt("question end", "missing"))
    };
    let state = match self.state.as_str() {
      "waiting" => QuestionState::Waiting,
      "interrupted" => QuestionState::Interrupted { at: ended()? },
      "answered" => QuestionState::Answered {
        answers: read_answers(&request, self.answers.as_deref())?,
        by: read_answerer("question answerer", self.answered_by.as_deref())?,
        at: ended()?,
      },
      other => return Err(corrupt("question state", other)),
    };
    let message = self
      .message
      .as_deref()
      .map(|message| parse("question answer message id", message))
      .transpose()?;
    Ok(AskedQuestions {
      request,
      state,
      message,
      orchestrator: self.orchestrator.from()?,
    })
  }
}

/// Stored answers `text` to `request`, checked against its questions.
fn read_answers(request: &QuestionRequest, text: Option<&str>) -> Result<Vec<Answer>, StoreError> {
  let text = text.ok_or_else(|| corrupt("question answers", "missing"))?;
  let given: Vec<GivenAnswer> = serde_json::from_str(text).map_err(|error| corrupt("question answers", error))?;
  request
    .answer(&given)
    .map_err(|error| corrupt("question answers", error))
}
