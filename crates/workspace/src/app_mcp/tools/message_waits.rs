//! The steps of runs that wait for a message, as `get_run` and `list_questions` give them (ORC-42): a step whose agent
//! ended it with a question in the conversation waits for the answer as a message, so it is listed with what its
//! agent wrote last ([`crate::tasks::last_words`]), and a wait is never unreadable. A step that waits on a question
//! with suggested answers is listed with that question instead.

use std::collections::BTreeMap;

use orchester_core::{id::ProjectId, question::AskedQuestions, run::Run, store::RunStore};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::wire;
use crate::{
  app_mcp::error::ToolError,
  links::TaskPage,
  tasks::{detail::run_store_error, last_words::waiting_words, recorded},
};

/// The steps of `run` that wait for a message, each with what its agent wrote last and `link`, the task's
/// Conversation where it shows; those waiting on a question of `asked` are left out.
pub fn message_waits(
  db: &ProjectDb,
  run: &Run,
  asked: &[AskedQuestions],
  link: Option<&str>,
) -> Result<Vec<Value>, ToolError> {
  let choosing = |execution| {
    asked
      .iter()
      .any(|question| question.waits() && question.request.execution() == execution)
  };
  let waits = waiting_words(db, run).map_err(ToolError::from)?;
  Ok(
    waits
      .into_iter()
      .filter(|(execution, _)| !choosing(*execution))
      .map(|(execution, words)| {
        let step = run.execution(execution).map(|found| found.step().clone());
        let name = step
          .as_ref()
          .and_then(|key| run.snapshot().body().step(key))
          .map(|pinned| pinned.name.as_str().to_owned());
        json!({
          "step": { "key": step.map(|key| key.to_string()), "name": name },
          "last_words": words.map(|words| json!({
            "text": &*words.text,
            "at": wire::time(words.at),
            "question": words.question,
          })),
          "link": link,
        })
      })
      .collect(),
  )
}

/// The steps of the latest runs of `project`'s tasks that wait for a message (only `only`'s when it names a run),
/// each with its run and task as `list_questions` lists them. Blocks on SQLite.
pub fn project_message_waits(db: &ProjectDb, project: ProjectId, only: Option<&Run>) -> Result<Vec<Value>, ToolError> {
  let asked = db.waiting_questions()?;
  let mut waits = Vec::new();
  for task in recorded::all(db)? {
    let latest = db.runs_of_task(task.id()).map_err(run_store_error)?.pop();
    let Some(Ok(run)) = latest else {
      continue;
    };
    if only.is_some_and(|only| only.id() != run.id()) || run.awaiting_answer().is_empty() {
      continue;
    }
    let key = task.key().to_string();
    let link = wire::task_link(project, &key, TaskPage::Conversation);
    waits.extend(run_waits(db, &run, &asked, (Some(&key), link.as_deref()))?);
  }
  // The newest run of each team task that no record of this computer defines waits without the task's key.
  let mut newest = BTreeMap::new();
  for run in recorded::unrecorded_runs(db)? {
    newest.insert(run.task(), run);
  }
  for run in newest.into_values() {
    if only.is_none_or(|only| only.id() == run.id()) && !run.awaiting_answer().is_empty() {
      waits.extend(run_waits(db, &run, &asked, (None, None))?);
    }
  }
  Ok(waits)
}

/// The waits of `run` as `list_questions` lists them, each with its run and its task's key (`None` and a null task for
/// a task that is `unrecorded`).
fn run_waits(
  db: &ProjectDb,
  run: &Run,
  asked: &[AskedQuestions],
  (key, link): (Option<&str>, Option<&str>),
) -> Result<Vec<Value>, ToolError> {
  let mut waits = message_waits(db, run, asked, link)?;
  for wait in &mut waits {
    wait["run"] = json!({ "id": run.id().to_string(), "number": run.number().get() });
    wait["task"] = json!(key);
    if key.is_none() {
      wait["unrecorded"] = json!(true);
    }
  }
  Ok(waits)
}
