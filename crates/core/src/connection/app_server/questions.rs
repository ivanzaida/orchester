//! Codex's own questions (`item/tool/requestUserInput` that is not an MCP tool's approval): one request
//! holds one or several questions, each with an id, its text, an optional header and suggested answers
//! (`isOther` lets the person answer in their own words, which Orchester always allows). The answer is the
//! request's response, `{"answers": {<question id>: {"answers": [<labels picked>, <own text>]}}}`, and Codex
//! goes on in the same turn. A question for a secret (`isSecret`) is never put to the person, and one
//! without two different suggested answers has no card: either is answered at once with why, and the
//! agent is told so.

use std::fmt;

use serde_json::{Map, Value, json};

use super::{ProtocolError, Update, string};
use crate::{
  connection::stream::QuestionRefusal,
  question::{Answer, Question, QuestionOption},
};

/// What the agent reads when its question asks for a secret.
pub const SECRET_REFUSED: &str = "Orchester never passes secret values through questions, so this question was not \
                                  shown. Ask the person to store the value in Project settings › Environment and grant it to \
                                  this run; do not ask for it again.";
/// What the agent reads when its question suggests no two different answers.
pub const NO_OPTIONS_REFUSED: &str = "Orchester puts a question to the person only with at least two different \
                                      suggested answers, so this question was not shown. Ask again with options, or \
                                      end your turn with your question.";

/// Questions of one request waiting for the person's answers.
#[derive(Clone, PartialEq)]
pub struct PendingQuestion {
  id: Value,
  pub thread: String,
  pub turn: Option<String>,
  /// The ids of the questions, in the request's order.
  keys: Vec<String>,
  questions: Vec<Question>,
}

impl fmt::Debug for PendingQuestion {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("PendingQuestion")
      .field("thread", &self.thread)
      .field("questions", &self.questions.len())
      .finish_non_exhaustive()
  }
}

impl PendingQuestion {
  pub fn questions(&self) -> &[Question] {
    &self.questions
  }

  /// Stable within one invocation; the invocation ID is prefixed by the runner for storage.
  pub fn request_key(&self) -> String {
    self.id.to_string()
  }

  /// The JSON-RPC response that gives `answers`, one per question in order.
  pub fn response(&self, answers: &[Answer]) -> String {
    let answers: Map<String, Value> = self
      .keys
      .iter()
      .zip(answers)
      .map(|(key, answer)| (key.clone(), json!({"answers": answer.values()})))
      .collect();
    json!({"id": self.id, "result": {"answers": answers}}).to_string()
  }
}

/// The update a question request `id` with `params` makes: questions for the person, or a refusal with the
/// response that tells the agent why.
pub(super) fn asked(id: &Value, params: &Value) -> Result<Update, ProtocolError> {
  let questions = params
    .get("questions")
    .and_then(Value::as_array)
    .filter(|questions| !questions.is_empty())
    .ok_or(ProtocolError::InvalidApproval)?;
  let keys: Vec<String> = questions
    .iter()
    .map(|question| string(question, "id").ok_or(ProtocolError::InvalidApproval))
    .collect::<Result<_, _>>()?;
  let named = |question: &Value| {
    string(question, "header")
      .filter(|header| !header.trim().is_empty())
      .or_else(|| string(question, "question"))
      .unwrap_or_default()
  };
  if let Some(secret) = questions
    .iter()
    .find(|question| question.get("isSecret").and_then(Value::as_bool).unwrap_or(false))
  {
    return Ok(refused(id, &keys, (named(secret), QuestionRefusal::Secret)));
  }
  let mut read = Vec::with_capacity(questions.len());
  for question in questions {
    match one(question) {
      Some(question) => read.push(question),
      None => return Ok(refused(id, &keys, (named(question), QuestionRefusal::NoOptions))),
    }
  }
  Ok(Update::Question(PendingQuestion {
    id: id.clone(),
    thread: string(params, "threadId").ok_or(ProtocolError::InvalidApproval)?,
    turn: string(params, "turnId"),
    keys,
    questions: read,
  }))
}

/// One question as Orchester shows it; `None` when it suggests no two different answers.
fn one(question: &Value) -> Option<Question> {
  let options = question
    .get("options")
    .and_then(Value::as_array)
    .map(|options| {
      options
        .iter()
        .filter_map(|option| {
          let label = option.get("label").and_then(Value::as_str)?;
          Some(QuestionOption::new(
            label,
            option.get("description").and_then(Value::as_str),
          ))
        })
        .collect()
    })
    .unwrap_or_default();
  let text = string(question, "question").unwrap_or_default();
  Question::new(&text, string(question, "header").as_deref(), options, false).ok()
}

/// The refusal of request `id`, whose questions are `keys`: every question answered with why.
fn refused(id: &Value, keys: &[String], (question, reason): (String, QuestionRefusal)) -> Update {
  let told = match reason {
    QuestionRefusal::Secret => SECRET_REFUSED,
    QuestionRefusal::NoOptions => NO_OPTIONS_REFUSED,
  };
  let answers: Map<String, Value> = keys
    .iter()
    .map(|key| (key.clone(), json!({"answers": [told]})))
    .collect();
  Update::QuestionRefused {
    response: json!({"id": id, "result": {"answers": answers}}).to_string(),
    question,
    reason,
  }
}
