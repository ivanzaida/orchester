//! The background operations of skills and repository agent files (ORC-15), as
//! [`super::operations`] describes the others.

use std::path::Path;

use orchester_core::id::ResourceId;

use super::{
  log::{OperationSpec, Place, Target},
  operations::{Wording, in_project},
};
use crate::{messages::Message, workspace::OpenProject};

/// Autosave of a skill draft; only its outcome is listed.
pub fn skill_draft(open: &OpenProject, id: ResourceId, name: &str) -> OperationSpec {
  Wording {
    running: "statusBar.operations.skillDraft.running",
    done: "statusBar.operations.skillDraft.done",
    failed: "statusBar.operations.skillDraft.failed",
  }
  .named(name)
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Skill(id)))
  .quiet()
}

pub fn skill_publish(open: &OpenProject, id: ResourceId, name: &str, version: u32) -> OperationSpec {
  let message = |key| {
    Message::new(key)
      .with("name", name.to_owned())
      .with("version", version.to_string())
  };
  OperationSpec::new(
    message("statusBar.operations.skillPublish.running"),
    message("statusBar.operations.skillPublish.done"),
    message("statusBar.operations.skillPublish.failed"),
  )
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Skill(id)))
}

/// Creating a skill; it opens the new skill once it exists (see [`skill`]).
pub fn skill_create(open: &OpenProject, name: &str) -> OperationSpec {
  Wording {
    running: "statusBar.operations.skillCreate.running",
    done: "statusBar.operations.skillCreate.done",
    failed: "statusBar.operations.skillCreate.failed",
  }
  .named(name)
  .project(open.project.name().as_str())
}

/// Reading a reference file from disk into the project's blobs.
pub fn skill_file_add(open: &OpenProject, id: ResourceId, name: &str) -> OperationSpec {
  Wording {
    running: "statusBar.operations.skillFileAdd.running",
    done: "statusBar.operations.skillFileAdd.done",
    failed: "statusBar.operations.skillFileAdd.failed",
  }
  .named(name)
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Skill(id)))
}

/// The target of a created skill in the project at `repo_root`.
pub fn skill(repo_root: &Path, id: ResourceId) -> Target {
  Target::Project {
    repo_root: repo_root.to_path_buf(),
    place: Place::Skill(id),
  }
}

/// Creating drafts from the repository's own agent files.
pub fn repository_import(open: &OpenProject, count: usize) -> OperationSpec {
  let message = |key| Message::new(key).with("count", count.to_string());
  OperationSpec::new(
    message("statusBar.operations.repositoryImport.running"),
    message("statusBar.operations.repositoryImport.done"),
    message("statusBar.operations.repositoryImport.failed"),
  )
  .project(open.project.name().as_str())
}

/// Saving Project settings › Context › Repository agent files; only its outcome is listed.
pub fn agent_files_setting(open: &OpenProject) -> OperationSpec {
  Wording {
    running: "statusBar.operations.agentFiles.running",
    done: "statusBar.operations.agentFiles.done",
    failed: "statusBar.operations.agentFiles.failed",
  }
  .unnamed()
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Settings))
  .quiet()
}
