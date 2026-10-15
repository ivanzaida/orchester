//! A run's waiting questions with suggested answers, as `get_run` lists them, `list_questions`, the waiting
//! questions of a run, a project or every project, and `answer_question`, which
//! answers one as the card in the task conversation does: through the runner, which gives the answer to
//! the asking agent. The answer is checked first, with the card's own rules: only the question's options,
//! one answer to a single-choice question, never an empty one.

use orchester_core::{
  id::ProjectId,
  question::{AnswerError, AskedQuestions, GivenAnswer, Question, QuestionTurn},
  run::Answerer,
  store::{RunStore, TaskStore},
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  Translate,
  across::{across, needs_project, oldest_first},
  runs::find_run,
  wire,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  links::TaskPage,
  messages::Message,
  tasks::{
    question::{Answering, answer_error},
    run::control_message,
  },
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct AnswerQuestion {
  /// The project's id, or its name.
  pub project: String,
  /// The run's id, or its number (`7` or `#7`).
  pub run: String,
  /// The question's id, as `get_run` lists it; left out, the run's only waiting question.
  #[serde(default)]
  pub question: Option<String>,
  /// The labels of the options picked, as `get_run` lists them.
  #[serde(default)]
  pub choices: Vec<String>,
  /// An answer in your own words, besides the options picked or instead of them.
  #[serde(default)]
  pub text: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ListQuestions {
  /// The project's id, or its name; left out, every project the app knows.
  #[serde(default)]
  pub project: Option<String>,
  /// Only this run's questions: its id, or its number (`7` or `#7`). Needs `project`.
  #[serde(default)]
  pub run: Option<String>,
}

/// A waiting question with its run's number and its task's key, while the run can be read.
struct Asked {
  asked: AskedQuestions,
  run: Option<u32>,
  task: Option<String>,
}

pub async fn list_questions(
  services: &Services,
  args: ListQuestions,
  translate: &Translate,
) -> Result<Value, ToolError> {
  if args.run.is_some() && args.project.is_none() {
    return Err(needs_project("run"));
  }
  let services = services.clone();
  let read = blocking(move || {
    across(&services, args.project.as_deref(), |_, db| {
      let run = args.run.as_deref().map(|named| find_run(db, named)).transpose()?;
      let waiting = db.waiting_questions()?.into_iter();
      Ok(
        waiting
          .filter(|asked| run.as_ref().is_none_or(|run| asked.request.run() == run.id()))
          .map(|asked| with_run(db, asked))
          .collect::<Vec<_>>(),
      )
    })
  })
  .await?;
  let mut questions = read.items(|open, asked| asked.iter().map(|asked| listed(asked, open.project.id())).collect());
  oldest_first(&mut questions, "asked_at");
  Ok(json!({ "questions": questions, "unreadable_projects": read.unreadable(translate) }))
}

/// `asked` with its run's number and task. A run or task that cannot be read leaves them out and the question is
/// still listed: `get_run` names why its run cannot be read.
fn with_run(db: &ProjectDb, asked: AskedQuestions) -> Asked {
  let Ok(Some(run)) = db.run(asked.request.run()) else {
    return Asked {
      asked,
      run: None,
      task: None,
    };
  };
  let task = match db.task(run.task()) {
    Ok(task) => task.map(|task| task.key().to_string()),
    Err(_) => None,
  };
  Asked {
    asked,
    run: Some(run.number().get()),
    task,
  }
}

/// A waiting question of `project` as `list_questions` lists it, with its run, its task and the link to its card.
fn listed(asked: &Asked, project: ProjectId) -> Value {
  let mut value = shown(&asked.asked);
  value["run"] = json!({ "id": asked.asked.request.run().to_string(), "number": asked.run });
  value["task"] = json!(asked.task);
  value["link"] = json!(
    asked
      .task
      .as_deref()
      .and_then(|key| wire::task_link(project, key, TaskPage::Conversation))
  );
  value
}

/// The run's questions that wait for an answer, each with its options and `link`, the task's Conversation
/// where their cards are.
pub fn waiting(questions: &[AskedQuestions], link: &str) -> Vec<Value> {
  questions
    .iter()
    .filter(|asked| asked.waits())
    .map(|asked| {
      let mut shown = shown(asked);
      shown["link"] = json!(link);
      shown
    })
    .collect()
}

fn shown(asked: &AskedQuestions) -> Value {
  let request = &asked.request;
  let questions: Vec<Value> = request.questions().iter().map(question).collect();
  json!({
    "id": request.id(),
    "agent": request.agent(),
    "asked_at": wire::time(request.asked_at()),
    // In its turn, the agent waits for the answer; otherwise its step ended with the question.
    "agent_waits_in_turn": request.turn() == QuestionTurn::InTurn,
    "questions": questions,
  })
}

fn question(question: &Question) -> Value {
  let options: Vec<Value> = question
    .options()
    .iter()
    .map(|option| json!({ "label": option.label(), "description": option.description() }))
    .collect();
  json!({
    "text": question.text(),
    "header": question.header(),
    "multi_select": question.multi_select(),
    "options": options,
    "own_answer": true,
  })
}

pub async fn answer_question(services: &Services, args: AnswerQuestion) -> Result<Value, ToolError> {
  let reading = services.clone();
  let AnswerQuestion {
    project,
    run,
    question,
    choices,
    text,
  } = args;
  let (project, asked) = blocking(move || {
    let project = reading.project(&project)?;
    let db = ProjectDb::open(&project.repo_root)?;
    let run = find_run(&db, &run)?;
    let asked = waiting_request(&db.questions(run.id())?, question.as_deref())?;
    Ok((project, asked))
  })
  .await?;
  let given = GivenAnswer { choices, text };
  let [one] = asked.request.questions() else {
    return Err(ToolError::refused(
      "several_questions",
      vec![Message::new("appMcp.errors.severalQuestions")],
    ));
  };
  one
    .answer(&given.choices, given.text.as_deref())
    .map_err(|error| refused(&error))?;
  let answering = Answering {
    run: asked.request.run(),
    request: asked.request.id().to_owned(),
    answers: vec![given],
  };
  services
    .runners
    .control_run_in(&project, answering.command(Answerer::Automation))
    .await
    .map_err(|error| ToolError::runner(&error, vec![control_message(&error)]))?;
  Ok(json!({ "question": { "id": asked.request.id(), "answered": true } }))
}

/// The waiting request named `id`, or the run's only one when none is named.
fn waiting_request(questions: &[AskedQuestions], id: Option<&str>) -> Result<AskedQuestions, ToolError> {
  let waiting: Vec<&AskedQuestions> = questions.iter().filter(|asked| asked.waits()).collect();
  match (id, waiting.as_slice()) {
    (Some(id), _) => waiting
      .iter()
      .find(|asked| asked.request.id() == id)
      .map(|asked| (*asked).clone())
      .ok_or_else(|| ToolError::not_found(Missing::Question, id)),
    (None, [only]) => Ok((*only).clone()),
    (None, []) => Err(ToolError::refused(
      "no_waiting_question",
      vec![Message::new("appMcp.errors.noWaitingQuestion")],
    )),
    (None, _) => Err(ToolError::refused(
      "question_ambiguous",
      vec![Message::new("appMcp.errors.questionAmbiguous")],
    )),
  }
}

/// An answer the question does not take, with the card's words for why.
fn refused(error: &AnswerError) -> ToolError {
  let code = match error {
    AnswerError::Empty => "empty_answer",
    AnswerError::UnknownChoice { .. } => "unknown_choice",
    AnswerError::RepeatedChoice { .. } => "repeated_choice",
    AnswerError::SeveralForSingle => "several_choices",
    AnswerError::AnswerCount { .. } => "answer_count",
  };
  ToolError::refused(code, vec![answer_error(error)])
}
