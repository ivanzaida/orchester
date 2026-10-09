//! Edits that are not saved, kept on this computer (`Leaving with unsaved changes` `Fd1ai`): leaving the
//! form never asks and never loses them. They are kept in the application database's window state
//! (`ui_state`) under a key of this app's own per project and task, with the version of the task they
//! were made on, and filled in again when the form opens there, later or after a restart. A save on that
//! version then meets any change made since, so kept edits never overwrite a newer version unseen. A
//! save or a discard removes them, and opening a form removes those of tasks that no longer exist.
//!
//! Writes wait until the typing pauses, run off the UI thread one after another, and are flushed when the
//! form closes.

use std::{
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
  },
  time::Duration,
};

use orchester_core::{
  id::{ProjectId, ResourceId, TaskId},
  sharing::TaskRevision,
  time::Timestamp,
};
use orchester_workspace::workspace::Workspace;
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::values::Values;

/// How long typing pauses before the edits are written.
const PAUSE: Duration = Duration::from_millis(500);
/// The key of New task's kept edits, after the project's prefix.
const NEW: &str = "new";

/// The version edits were made on: its revision (none for a task not created yet) and its values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeptBase {
  pub revision: Option<TaskRevision>,
  pub values: Values,
}

/// Edits kept from an earlier visit, the version they were made on, and when they were kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
  pub values: Values,
  pub base: KeptBase,
  pub at: Timestamp,
}

impl lurq::app::component::DevtoolsInspectable for Kept {}

/// Where one form keeps its edits.
#[derive(Clone)]
pub struct KeptEdits {
  inner: Arc<Inner>,
}

struct Inner {
  project: ProjectId,
  /// The form that writes. When its edits come back to what is stored, it removes them only if it
  /// kept them itself, so a tab whose edits were undone does not remove another tab's.
  owner: Arc<str>,
  key: Mutex<Arc<str>>,
  workspace: Workspace,
  runtime: Handle,
  /// The write asked for last that is not done yet, and its key.
  pending: Mutex<Option<(Arc<str>, Write)>>,
  /// Counts the writes asked for, so a pause that ends after a newer one writes nothing.
  asked: AtomicU64,
  /// A write runs; it takes the pending one after it.
  writing: AtomicBool,
}

/// What a write does.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Write {
  Keep(String),
  /// Removes what is kept, if this form kept it: its edits came back to what is stored.
  Undone,
  /// Removes what is kept whoever kept it: it was saved, discarded, or cannot be saved any more.
  Remove,
}

impl KeptEdits {
  /// The edits of `task` of `project`; `None` is New task's.
  pub fn new(workspace: Workspace, runtime: Handle, (project, task): (ProjectId, Option<TaskId>), owner: &str) -> Self {
    Self {
      inner: Arc::new(Inner {
        owner: owner.into(),
        key: Mutex::new(key(project, task)),
        project,
        workspace,
        runtime,
        pending: Mutex::new(None),
        asked: AtomicU64::new(0),
        writing: AtomicBool::new(false),
      }),
    }
  }

  /// The edits kept here, if any, after removing the kept edits of the project's tasks that are not in
  /// `tasks` any more. Blocks on SQLite. Text that cannot be read as kept edits is logged and reads as
  /// none.
  pub fn read(&self, tasks: &[TaskId]) -> Option<Kept> {
    self.prune(tasks);
    let key = self.inner.key.lock().clone();
    let text = match self.inner.workspace.ui_state(&key) {
      Ok(text) => text?,
      Err(error) => {
        tracing::warn!(%error, %key, "cannot read the kept edits");
        return None;
      }
    };
    let kept = serde_json::from_str::<Value>(&text)
      .ok()
      .and_then(|value| decode(&value));
    if kept.is_none() {
      tracing::warn!(%key, "kept edits that cannot be read are left out");
    }
    kept
  }

  fn prune(&self, tasks: &[TaskId]) {
    let prefix = prefix(self.inner.project);
    let keys = match self.inner.workspace.ui_state_keys(&prefix) {
      Ok(keys) => keys,
      Err(error) => {
        tracing::warn!(%error, "cannot list the kept edits");
        return;
      }
    };
    for key in keys {
      let task = key[prefix.len()..].parse::<TaskId>().ok();
      let gone = task.is_some_and(|task| !tasks.contains(&task));
      if gone && let Err(error) = self.inner.workspace.remove_ui_state(&key) {
        tracing::warn!(%error, %key, "cannot remove kept edits of a task that is gone");
      }
    }
  }

  /// Keeps `values`, made on `base`, as of `at`, or, with `None`, removes what this form kept: its edits
  /// came back to what is stored. Once the typing pauses.
  pub fn write(&self, values: Option<(&Values, &KeptBase, Timestamp)>) {
    let write = values.map_or(Write::Undone, |(values, base, at)| {
      Write::Keep(encode(values, base, (at, &self.inner.owner)).to_string())
    });
    self.ask(write);
  }

  /// Removes what is kept, whoever kept it, at once: the edits were saved or discarded, or the task
  /// cannot take them.
  pub fn remove(&self) {
    let key = self.inner.key.lock().clone();
    *self.inner.pending.lock() = Some((key, Write::Remove));
    self.inner.asked.fetch_add(1, Ordering::SeqCst);
    self.flush();
  }

  fn ask(&self, write: Write) {
    let key = self.inner.key.lock().clone();
    *self.inner.pending.lock() = Some((key, write));
    let asked = self.inner.asked.fetch_add(1, Ordering::SeqCst) + 1;
    let edits = self.clone();
    self.inner.runtime.spawn(async move {
      tokio::time::sleep(PAUSE).await;
      if edits.inner.asked.load(Ordering::SeqCst) == asked {
        edits.flush();
      }
    });
  }

  /// Removes what is kept now and keeps the form's edits under `task` from here: New task created it.
  pub fn move_to(&self, task: TaskId) {
    self.remove();
    *self.inner.key.lock() = key(self.inner.project, Some(task));
  }

  /// Writes the pending edits now, e.g. as the form closes.
  pub fn flush(&self) {
    if self.inner.writing.swap(true, Ordering::SeqCst) {
      return;
    }
    let inner = self.inner.clone();
    self.inner.runtime.spawn_blocking(move || {
      loop {
        let next = inner.pending.lock().take();
        if let Some((key, write)) = next {
          persist(&inner, &key, &write);
          continue;
        }
        inner.writing.store(false, Ordering::SeqCst);
        // A write asked for between the take and the store above is taken here, unless another flush
        // took over.
        if inner.pending.lock().is_none() || inner.writing.swap(true, Ordering::SeqCst) {
          return;
        }
      }
    });
  }
}

fn persist(inner: &Inner, key: &str, write: &Write) {
  let workspace = &inner.workspace;
  let done = match write {
    Write::Keep(text) => workspace.set_ui_state(key, text),
    Write::Undone if kept_by_another(workspace, key, &inner.owner) => return,
    Write::Undone | Write::Remove => workspace.remove_ui_state(key),
  };
  if let Err(error) = done {
    tracing::warn!(%error, %key, "cannot keep the form's edits");
  }
}

/// Whether what is kept under `key` was written by another form than `owner`, e.g. another tab's.
fn kept_by_another(workspace: &Workspace, key: &str, owner: &str) -> bool {
  let Ok(Some(text)) = workspace.ui_state(key) else {
    return false;
  };
  let written_by = serde_json::from_str::<Value>(&text)
    .ok()
    .and_then(|value| value.get("owner")?.as_str().map(str::to_owned));
  written_by.is_some_and(|written_by| written_by != owner)
}

fn prefix(project: ProjectId) -> String {
  format!("orchester-app.task_form.kept.{project}.")
}

fn key(project: ProjectId, task: Option<TaskId>) -> Arc<str> {
  let task = task.map_or_else(|| NEW.to_owned(), |task| task.to_string());
  format!("{}{task}", prefix(project)).into()
}

fn encode_values(values: &Values) -> Value {
  json!({
    "title": values.title,
    "brief": values.brief,
    "criteria": values.criteria,
    "pipeline": values.pipeline.map(|pipeline| pipeline.to_string()),
    "budget": values.budget,
    "time_limit": values.time_limit,
  })
}

fn encode(values: &Values, base: &KeptBase, (at, owner): (Timestamp, &str)) -> Value {
  json!({
    "at": at.unix_millis(),
    "owner": owner,
    "values": encode_values(values),
    "base_revision": base.revision.map(TaskRevision::get),
    "base": encode_values(&base.values),
  })
}

fn decode_values(value: &Value) -> Option<Values> {
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
  Some(Values {
    title: text("title")?,
    brief: text("brief")?,
    criteria,
    pipeline,
    budget: text("budget")?,
    time_limit: text("time_limit")?,
  })
}

fn decode(value: &Value) -> Option<Kept> {
  let revision = match value.get("base_revision")? {
    Value::Null => None,
    revision => Some(TaskRevision::try_from(revision.as_u64()?).ok()?),
  };
  Some(Kept {
    values: decode_values(value.get("values")?)?,
    base: KeptBase {
      revision,
      values: decode_values(value.get("base")?)?,
    },
    at: Timestamp::from_unix_millis(value.get("at")?.as_i64()?),
  })
}
