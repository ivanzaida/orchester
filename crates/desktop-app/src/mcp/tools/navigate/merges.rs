//! Review › Merges, or with `key` and `run` one run's attempts under it.

use std::num::NonZeroU32;

use orchester_core::run::RunNumber;
use serde_json::{Map, Value};

use super::{ToolError, key, string};
use crate::routes::ProjectPage;

pub(super) const PARAMS: &[&str] = &["project", "key", "run"];

/// Merges, or the attempt page of run `run` of the task `key`; one without the other is refused.
pub(super) fn merges_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  match (string(args, "key")?, string(args, "run")?) {
    (None, None) => Ok(ProjectPage::Merges),
    (Some(_), None) => Err(ToolError::Missing { param: "run" }),
    (None, Some(_)) => Err(ToolError::Missing { param: "key" }),
    (Some(_), Some(number)) => {
      let number = number
        .trim_start_matches('#')
        .parse::<NonZeroU32>()
        .map_err(|_| ToolError::InvalidParam {
          param: "run",
          reason: format!("`{number}` is not a run's number"),
        })?;
      Ok(ProjectPage::MergeAttempt(key(args, "key")?, RunNumber::new(number)))
    }
  }
}
