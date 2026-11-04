//! The background operations of Project settings › Context › Code index, as [`super::operations`] describes
//! the others.

use super::{
  log::{OperationSpec, Place},
  operations::{Wording, in_project},
};
use crate::workspace::OpenProject;

/// Saving the Use code index setting; only its outcome is listed.
pub fn code_index_setting(open: &OpenProject) -> OperationSpec {
  Wording {
    running: "statusBar.operations.codeIndex.running",
    done: "statusBar.operations.codeIndex.done",
    failed: "statusBar.operations.codeIndex.failed",
  }
  .unnamed()
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Settings))
  .quiet()
}

/// Rebuilding the index of the revision `short` names.
pub fn code_index_rebuild(open: &OpenProject, short: &str) -> OperationSpec {
  Wording {
    running: "statusBar.operations.codeIndexRebuild.running",
    done: "statusBar.operations.codeIndexRebuild.done",
    failed: "statusBar.operations.codeIndexRebuild.failed",
  }
  .named(short)
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Settings))
}
