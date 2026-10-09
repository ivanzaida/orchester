//! Edits that are not saved, kept on this computer (`Leaving with unsaved changes` `Fd1ai`): leaving the
//! form never asks and never loses them. They are kept in the application database's window state
//! (`ui_state`) under a key of this app's own per project and task, and filled in again when the form
//! opens there, later or after a restart, with the time they were kept. A save or a discard clears them.

use std::sync::{
  Arc,
  atomic::{AtomicU64, Ordering},
};

use orchester_core::{
  id::{ProjectId, ResourceId, TaskId},
  time::Timestamp,
};
use orchester_workspace::workspace::Workspace;
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::values::Values;

/// Edits kept from an earlier visit, and when they were kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
  pub values: Values,
  pub at: Timestamp,
}

impl lurq::app::component::DevtoolsInspectable for Kept {}

/// Where one form keeps its edits. Writes run off the UI thread, in order: one that would land after a
/// newer one is dropped.
#[derive(Clone)]
pub struct KeptEdits {
  key: Arc<str>,
  workspace: Workspace,
  runtime: Handle,
  /// The number of the latest write asked for, and of the latest one done.
  asked: Arc<AtomicU64>,
  done: Arc<Mutex<u64>>,
}

impl KeptEdits {
  /// The edits of `task` of `project`; `None` is New task's.
  pub fn new(workspace: Workspace, runtime: Handle, project: ProjectId, task: Option<TaskId>) -> Self {
    let task = task.map_or_else(|| "new".to_owned(), |task| task.to_string());
    Self {
      key: format!("orchester-app.task_form.kept.{project}.{task}").into(),
      workspace,
      runtime,
      asked: Arc::default(),
      done: Arc::default(),
    }
  }

  /// The edits kept here, if any. Blocks on SQLite. Text that cannot be read as kept edits is logged and
  /// reads as none: nothing is lost that a person could get back.
  pub fn read(&self) -> Option<Kept> {
    let text = match self.workspace.ui_state(&self.key) {
      Ok(text) => text?,
      Err(error) => {
        tracing::warn!(%error, key = %self.key, "cannot read the kept edits");
        return None;
      }
    };
    if text.is_empty() {
      return None;
    }
    let kept = serde_json::from_str::<Value>(&text)
      .ok()
      .and_then(|value| decode(&value));
    if kept.is_none() {
      tracing::warn!(key = %self.key, "kept edits that cannot be read are left out");
    }
    kept
  }

  /// Keeps `values` as of `at`, or clears what is kept when `None`.
  pub fn write(&self, values: Option<(&Values, Timestamp)>) {
    let text = values.map_or_else(String::new, |(values, at)| encode(values, at).to_string());
    let number = self.asked.fetch_add(1, Ordering::SeqCst) + 1;
    let (key, workspace, done) = (self.key.clone(), self.workspace.clone(), self.done.clone());
    // Spawned rather than awaited: the write must land also when the form closes right after it.
    self.runtime.spawn_blocking(move || {
      let mut done = done.lock();
      if *done > number {
        return;
      }
      if let Err(error) = workspace.set_ui_state(&key, &text) {
        tracing::warn!(%error, %key, "cannot keep the form's edits");
      }
      *done = number;
    });
  }
}

fn encode(values: &Values, at: Timestamp) -> Value {
  json!({
    "at": at.unix_millis(),
    "title": values.title,
    "brief": values.brief,
    "criteria": values.criteria,
    "pipeline": values.pipeline.map(|pipeline| pipeline.to_string()),
    "budget": values.budget,
    "time_limit": values.time_limit,
  })
}

fn decode(value: &Value) -> Option<Kept> {
  let text = |name: &str| value.get(name)?.as_str().map(str::to_owned);
  let pipeline = match value.get("pipeline")? {
    Value::Null => None,
    Value::String(id) => Some(id.parse::<ResourceId>().ok()?),
    _ => return None,
  };
  let criteria = value
    .get("criteria")?
    .as_array()?
    .iter()
    .map(|criterion| criterion.as_str().map(str::to_owned))
    .collect::<Option<Vec<_>>>()?;
  Some(Kept {
    values: Values {
      title: text("title")?,
      brief: text("brief")?,
      criteria,
      pipeline,
      budget: text("budget")?,
      time_limit: text("time_limit")?,
    },
    at: Timestamp::from_unix_millis(value.get("at")?.as_i64()?),
  })
}
