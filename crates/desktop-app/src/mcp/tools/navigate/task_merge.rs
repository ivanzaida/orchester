//! A task's page, or with `run` one run's merge attempts under it.

use std::num::NonZeroU32;

use orchester_core::run::RunNumber;
use serde_json::{Map, Value};

use super::{ToolError, key, string, task_view};
use crate::routes::ProjectPage;

/// What a task's page takes besides `project`; `run` goes with `key` alone.
pub(super) const PARAMS: &[&str] = &["project", "key", "tab", "view", "file", "from", "run"];

/// The page of the task `key`, or the merge attempts of its run `run`, which no tab, view or file goes with.
pub(super) fn task_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  let Some(number) = string(args, "run")? else {
    return Ok(ProjectPage::Task(key(args, "key")?, task_view(args)?));
  };
  if let Some(param) = ["tab", "view", "file", "from"]
    .into_iter()
    .find(|param| args.contains_key(*param))
  {
    return Err(ToolError::InvalidParam {
      param: "run",
      reason: format!("a run's merge attempts have no `{param}`"),
    });
  }
  let number = number
    .trim_start_matches('#')
    .parse::<NonZeroU32>()
    .map_err(|_| ToolError::InvalidParam {
      param: "run",
      reason: format!("`{number}` is not a run's number"),
    })?;
  Ok(ProjectPage::TaskMerge(key(args, "key")?, RunNumber::new(number)))
}
