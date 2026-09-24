//! The background operations the desktop reports, with their copy (`statusBar.operations.*`)
//! and what opening them shows. Every one has a real start and end in the screen that runs it.

use std::path::Path;

use orchester_core::{connection::AgentProvider, id::ResourceId};

use super::log::{OperationSpec, Place, Target};
use crate::{agents::data::ProfileChange, messages::Message, workspace::OpenProject};

/// Copy keys of one operation: while it runs, once it finished, and its failure heading.
struct Wording {
  running: &'static str,
  done: &'static str,
  failed: &'static str,
}

impl Wording {
  /// The operation on the object called `name`.
  fn named(&self, name: &str) -> OperationSpec {
    let message = |key| Message::new(key).with("name", name.to_owned());
    OperationSpec::new(message(self.running), message(self.done), message(self.failed))
  }

  fn unnamed(&self) -> OperationSpec {
    OperationSpec::new(
      Message::new(self.running),
      Message::new(self.done),
      Message::new(self.failed),
    )
  }
}

fn in_project(open: &OpenProject, place: Place) -> Target {
  Target::Project {
    repo_root: open.repo_root.clone(),
    place,
  }
}

/// Autosave of a pipeline draft; only its outcome is listed.
pub fn pipeline_draft(open: &OpenProject, id: ResourceId, name: &str) -> OperationSpec {
  Wording {
    running: "statusBar.operations.pipelineDraft.running",
    done: "statusBar.operations.pipelineDraft.done",
    failed: "statusBar.operations.pipelineDraft.failed",
  }
  .named(name)
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Pipeline(id)))
  .quiet()
}

pub fn pipeline_publish(open: &OpenProject, id: ResourceId, name: &str, version: u32) -> OperationSpec {
  let message = |key| {
    Message::new(key)
      .with("name", name.to_owned())
      .with("version", version.to_string())
  };
  OperationSpec::new(
    message("statusBar.operations.pipelinePublish.running"),
    message("statusBar.operations.pipelinePublish.done"),
    message("statusBar.operations.pipelinePublish.failed"),
  )
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Pipeline(id)))
}

/// Creating a pipeline; it opens the new pipeline once it exists (see [`pipeline`]).
pub fn pipeline_create(open: &OpenProject, name: &str) -> OperationSpec {
  Wording {
    running: "statusBar.operations.pipelineCreate.running",
    done: "statusBar.operations.pipelineCreate.done",
    failed: "statusBar.operations.pipelineCreate.failed",
  }
  .named(name)
  .project(open.project.name().as_str())
}

/// The target of a created pipeline in the project at `repo_root`.
pub fn pipeline(repo_root: &Path, id: ResourceId) -> Target {
  Target::Project {
    repo_root: repo_root.to_path_buf(),
    place: Place::Pipeline(id),
  }
}

pub fn project_create(name: &str, repo_root: &Path) -> OperationSpec {
  Wording {
    running: "statusBar.operations.projectCreate.running",
    done: "statusBar.operations.projectCreate.done",
    failed: "statusBar.operations.projectCreate.failed",
  }
  .named(name)
  .project(name)
  .target(Target::Project {
    repo_root: repo_root.to_path_buf(),
    place: Place::Overview,
  })
}

/// The native folder dialog for the project repository, while it is open.
pub fn repository_pick() -> OperationSpec {
  Wording {
    running: "statusBar.operations.repositoryPick.running",
    done: "statusBar.operations.repositoryPick.done",
    failed: "statusBar.operations.repositoryPick.failed",
  }
  .unnamed()
}

/// Saving a new task as a draft; only its outcome is listed.
pub fn task_draft(open: &OpenProject, title: &str) -> OperationSpec {
  Wording {
    running: "statusBar.operations.taskDraft.running",
    done: "statusBar.operations.taskDraft.done",
    failed: "statusBar.operations.taskDraft.failed",
  }
  .named(title)
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Tasks))
  .quiet()
}

/// A change to an agent profile. Publishing and keeping a draft carry only the profile's id,
/// so their copy does not name it.
pub fn profile_change(open: &OpenProject, change: &ProfileChange) -> OperationSpec {
  let spec = match change {
    ProfileChange::Create(profile) => Wording {
      running: "statusBar.operations.profileCreate.running",
      done: "statusBar.operations.profileCreate.done",
      failed: "statusBar.operations.profileCreate.failed",
    }
    .named(profile.role().as_str()),
    ProfileChange::SaveDraft(_, profile) => Wording {
      running: "statusBar.operations.profileDraft.running",
      done: "statusBar.operations.profileDraft.done",
      failed: "statusBar.operations.profileDraft.failed",
    }
    .named(profile.role().as_str())
    .quiet(),
    ProfileChange::Publish(_) => Wording {
      running: "statusBar.operations.profilePublish.running",
      done: "statusBar.operations.profilePublish.done",
      failed: "statusBar.operations.profilePublish.failed",
    }
    .unnamed(),
    ProfileChange::KeepDraft(_) => Wording {
      running: "statusBar.operations.profileKeepDraft.running",
      done: "statusBar.operations.profileKeepDraft.done",
      failed: "statusBar.operations.profileKeepDraft.failed",
    }
    .unnamed(),
  };
  let spec = spec.project(open.project.name().as_str());
  match change {
    ProfileChange::Create(_) => spec,
    ProfileChange::SaveDraft(id, _) | ProfileChange::Publish(id) | ProfileChange::KeepDraft(id) => {
      spec.target(in_project(open, Place::AgentProfile(*id)))
    }
  }
}

/// The target of a created profile in the project at `repo_root`.
pub fn profile(repo_root: &Path, id: ResourceId) -> Target {
  Target::Project {
    repo_root: repo_root.to_path_buf(),
    place: Place::AgentProfile(id),
  }
}

/// Checking a saved connection again from its details.
pub fn connection_check(open: &OpenProject, name: &str) -> OperationSpec {
  Wording {
    running: "statusBar.operations.connectionCheck.running",
    done: "statusBar.operations.connectionCheck.done",
    failed: "statusBar.operations.connectionCheck.failed",
  }
  .named(name)
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Connections))
}

pub fn connection_save(open: &OpenProject, name: &str) -> OperationSpec {
  Wording {
    running: "statusBar.operations.connectionSave.running",
    done: "statusBar.operations.connectionSave.done",
    failed: "statusBar.operations.connectionSave.failed",
  }
  .named(name)
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Connections))
}

/// Checking a provider's CLI in the connection dialog; named by the product, e.g. "Codex CLI".
pub fn cli_check(open: &OpenProject, provider: AgentProvider) -> OperationSpec {
  let wording = match provider {
    AgentProvider::CodexCli => Wording {
      running: "statusBar.operations.cliCheck.codex.running",
      done: "statusBar.operations.cliCheck.codex.done",
      failed: "statusBar.operations.cliCheck.codex.failed",
    },
    AgentProvider::ClaudeCodeCli => Wording {
      running: "statusBar.operations.cliCheck.claude.running",
      done: "statusBar.operations.cliCheck.claude.done",
      failed: "statusBar.operations.cliCheck.claude.failed",
    },
  };
  wording
    .unnamed()
    .project(open.project.name().as_str())
    .target(in_project(open, Place::Connections))
}

/// The native file dialog for a provider's program, while it is open.
pub fn program_pick() -> OperationSpec {
  Wording {
    running: "statusBar.operations.programPick.running",
    done: "statusBar.operations.programPick.done",
    failed: "statusBar.operations.programPick.failed",
  }
  .unnamed()
}
