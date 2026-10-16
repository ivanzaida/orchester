//! The background operations the desktop reports, with their copy (`statusBar.operations.*`)
//! and what opening them shows. Every one has a real start and end in the screen that runs it.

use std::path::Path;

use orchester_core::{
  connection::AgentProvider,
  id::{ResourceId, TaskId},
};

use super::log::{OperationSpec, Place, Target};
use crate::{agents::data::ProfileChange, messages::Message, tools::data::ToolChange, workspace::OpenProject};

/// Copy keys of one operation: while it runs, once it finished, and its failure heading.
pub(super) struct Wording {
  pub(super) running: &'static str,
  pub(super) done: &'static str,
  pub(super) failed: &'static str,
}

impl Wording {
  /// The operation on the object called `name`.
  pub(super) fn named(&self, name: &str) -> OperationSpec {
    let message = |key| Message::new(key).with("name", name.to_owned());
    OperationSpec::new(message(self.running), message(self.done), message(self.failed))
  }

  pub(super) fn unnamed(&self) -> OperationSpec {
    OperationSpec::new(
      Message::new(self.running),
      Message::new(self.done),
      Message::new(self.failed),
    )
  }
}

pub(super) fn in_project(open: &OpenProject, place: Place) -> Target {
  Target::Project {
    repo_root: open.repo_root.clone(),
    place,
  }
}

/// Syncing the open pull requests of the project's repository with GitHub. Quiet: the Pull
/// requests screen syncs every 60 s while it is shown, so only the outcome is listed.
pub fn pull_request_sync(open: &OpenProject) -> OperationSpec {
  Wording {
    running: "statusBar.operations.pullRequestSync.running",
    done: "statusBar.operations.pullRequestSync.done",
    failed: "statusBar.operations.pullRequestSync.failed",
  }
  .unnamed()
  .project(open.project.name().as_str())
  .target(in_project(open, Place::PullRequests))
  .quiet()
}

/// Reading one pull request and its diff from GitHub. Quiet: the pull request screen reads it
/// every time it opens, so only the outcome is listed.
pub fn pull_request_refresh(open: &OpenProject, number: u64) -> OperationSpec {
  Wording {
    running: "statusBar.operations.pullRequestRefresh.running",
    done: "statusBar.operations.pullRequestRefresh.done",
    failed: "statusBar.operations.pullRequestRefresh.failed",
  }
  .named(&number.to_string())
  .project(open.project.name().as_str())
  .target(in_project(open, Place::PullRequests))
  .quiet()
}

/// Handing a pull request's address to the default browser.
pub fn pull_request_open(open: &OpenProject, number: u64) -> OperationSpec {
  Wording {
    running: "statusBar.operations.pullRequestOpen.running",
    done: "statusBar.operations.pullRequestOpen.done",
    failed: "statusBar.operations.pullRequestOpen.failed",
  }
  .named(&number.to_string())
  .project(open.project.name().as_str())
  .target(in_project(open, Place::PullRequests))
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

/// Importing `count` resources from the project called `source`; it names the first imported
/// resource once it exists.
pub fn import_resources(open: &OpenProject, source: &str, count: usize) -> OperationSpec {
  let wording = if count == 1 {
    Wording {
      running: "statusBar.operations.import.running.one",
      done: "statusBar.operations.import.done.one",
      failed: "statusBar.operations.import.failed.one",
    }
  } else {
    Wording {
      running: "statusBar.operations.import.running.other",
      done: "statusBar.operations.import.done.other",
      failed: "statusBar.operations.import.failed.other",
    }
  };
  let message = |key| {
    Message::new(key)
      .with("project", source.to_owned())
      .with("count", count.to_string())
  };
  OperationSpec::new(message(wording.running), message(wording.done), message(wording.failed))
    .project(open.project.name().as_str())
}

/// Autosave of an instruction draft; only its outcome is listed.
pub fn instruction_draft(open: &OpenProject, id: ResourceId, name: &str) -> OperationSpec {
  Wording {
    running: "statusBar.operations.instructionDraft.running",
    done: "statusBar.operations.instructionDraft.done",
    failed: "statusBar.operations.instructionDraft.failed",
  }
  .named(name)
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Instruction(id)))
  .quiet()
}

pub fn instruction_publish(open: &OpenProject, id: ResourceId, name: &str, version: u32) -> OperationSpec {
  let message = |key| {
    Message::new(key)
      .with("name", name.to_owned())
      .with("version", version.to_string())
  };
  OperationSpec::new(
    message("statusBar.operations.instructionPublish.running"),
    message("statusBar.operations.instructionPublish.done"),
    message("statusBar.operations.instructionPublish.failed"),
  )
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Instruction(id)))
}

/// Creating an instruction; it opens the new instruction once it exists (see [`instruction`]).
pub fn instruction_create(open: &OpenProject, name: &str) -> OperationSpec {
  Wording {
    running: "statusBar.operations.instructionCreate.running",
    done: "statusBar.operations.instructionCreate.done",
    failed: "statusBar.operations.instructionCreate.failed",
  }
  .named(name)
  .project(open.project.name().as_str())
}

/// The target of a created instruction in the project at `repo_root`.
pub fn instruction(repo_root: &Path, id: ResourceId) -> Target {
  Target::Project {
    repo_root: repo_root.to_path_buf(),
    place: Place::Instruction(id),
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

/// Merging run `number` of task `key` ("ORC-12", 3) into `branch`, then clearing what it left.
pub fn run_merge(open: &OpenProject, task: TaskId, (key, number): (&str, u32), branch: &str) -> OperationSpec {
  let message = |text| {
    Message::new(text)
      .with("task", key.to_owned())
      .with("run", number.to_string())
      .with("branch", branch.to_owned())
  };
  OperationSpec::new(
    message("statusBar.operations.runMerge.running"),
    message("statusBar.operations.runMerge.done"),
    message("statusBar.operations.runMerge.failed"),
  )
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Task(task)))
}

/// Removing the worktrees of run `number` of task `key` ("ORC-12", 3).
pub fn run_cleanup(open: &OpenProject, task: TaskId, (key, number): (&str, u32)) -> OperationSpec {
  let message = |text| {
    Message::new(text)
      .with("task", key.to_owned())
      .with("run", number.to_string())
  };
  OperationSpec::new(
    message("statusBar.operations.runCleanup.running"),
    message("statusBar.operations.runCleanup.done"),
    message("statusBar.operations.runCleanup.failed"),
  )
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Task(task)))
}

/// Pushing the branch of run `number` of task `key` ("ORC-12", 3) and opening a pull request from it.
pub fn run_pull_request(open: &OpenProject, task: TaskId, (key, number): (&str, u32)) -> OperationSpec {
  let message = |text| Message::new(text).with("task", key.to_owned()).with("run", number.to_string());
  OperationSpec::new(
    message("statusBar.operations.runPullRequest.running"),
    message("statusBar.operations.runPullRequest.done"),
    message("statusBar.operations.runPullRequest.failed"),
  )
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Task(task)))
}

/// Saving Project settings › Environment; only its outcome is listed.
pub fn project_environment(open: &OpenProject) -> OperationSpec {
  Wording {
    running: "statusBar.operations.projectEnvironment.running",
    done: "statusBar.operations.projectEnvironment.done",
    failed: "statusBar.operations.projectEnvironment.failed",
  }
  .unnamed()
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Settings))
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

const TOOL_CREATE: Wording = Wording {
  running: "statusBar.operations.toolCreate.running",
  done: "statusBar.operations.toolCreate.done",
  failed: "statusBar.operations.toolCreate.failed",
};
const TOOL_DRAFT: Wording = Wording {
  running: "statusBar.operations.toolDraft.running",
  done: "statusBar.operations.toolDraft.done",
  failed: "statusBar.operations.toolDraft.failed",
};
const TOOL_KEEP_DRAFT: Wording = Wording {
  running: "statusBar.operations.toolKeepDraft.running",
  done: "statusBar.operations.toolKeepDraft.done",
  failed: "statusBar.operations.toolKeepDraft.failed",
};

/// A change to the tool called `name`; publishing makes it version `version`. Saving a draft is
/// quiet: only its outcome is listed.
pub fn tool_change(open: &OpenProject, name: &str, change: &ToolChange, version: u32) -> OperationSpec {
  let spec = match change {
    ToolChange::Create(..) => TOOL_CREATE.named(name),
    ToolChange::SaveDraft(..) => TOOL_DRAFT.named(name).quiet(),
    ToolChange::KeepDraft(_) => TOOL_KEEP_DRAFT.named(name),
    ToolChange::Publish(_) => {
      let message = |key| {
        Message::new(key)
          .with("name", name.to_owned())
          .with("version", version.to_string())
      };
      OperationSpec::new(
        message("statusBar.operations.toolPublish.running"),
        message("statusBar.operations.toolPublish.done"),
        message("statusBar.operations.toolPublish.failed"),
      )
    }
  };
  let spec = spec.project(open.project.name().as_str());
  match change {
    ToolChange::Create(..) => spec,
    ToolChange::SaveDraft(id, _) | ToolChange::Publish(id) | ToolChange::KeepDraft(id) => {
      spec.target(in_project(open, Place::Tool(*id)))
    }
  }
}

/// The target of a created tool in the project at `repo_root`.
pub fn tool(repo_root: &Path, id: ResourceId) -> Target {
  Target::Project {
    repo_root: repo_root.to_path_buf(),
    place: Place::Tool(id),
  }
}

/// The target of a created profile in the project at `repo_root`.
pub fn profile(repo_root: &Path, id: ResourceId) -> Target {
  Target::Project {
    repo_root: repo_root.to_path_buf(),
    place: Place::AgentProfile(id),
  }
}

/// Adding a secret: its value goes to the keychain, its name to the project.
pub fn secret_add(open: &OpenProject, name: &str) -> OperationSpec {
  secret_operation(
    open,
    name,
    &Wording {
      running: "statusBar.operations.secretAdd.running",
      done: "statusBar.operations.secretAdd.done",
      failed: "statusBar.operations.secretAdd.failed",
    },
  )
}

/// Replacing a secret's value in the keychain (rotation).
pub fn secret_replace(open: &OpenProject, name: &str) -> OperationSpec {
  secret_operation(
    open,
    name,
    &Wording {
      running: "statusBar.operations.secretReplace.running",
      done: "statusBar.operations.secretReplace.done",
      failed: "statusBar.operations.secretReplace.failed",
    },
  )
}

/// Archiving a secret: its value is removed from the keychain.
pub fn secret_archive(open: &OpenProject, name: &str) -> OperationSpec {
  secret_operation(
    open,
    name,
    &Wording {
      running: "statusBar.operations.secretArchive.running",
      done: "statusBar.operations.secretArchive.done",
      failed: "statusBar.operations.secretArchive.failed",
    },
  )
}

fn secret_operation(open: &OpenProject, name: &str, wording: &Wording) -> OperationSpec {
  wording
    .named(name)
    .project(open.project.name().as_str())
    .target(in_project(open, Place::Secrets))
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

/// Turning MCP server `name` on (`enabled`) or off.
pub fn mcp_server_switch(open: &OpenProject, name: &str, enabled: bool) -> OperationSpec {
  let wording = if enabled {
    Wording {
      running: "statusBar.operations.mcpServerOn.running",
      done: "statusBar.operations.mcpServerOn.done",
      failed: "statusBar.operations.mcpServerOn.failed",
    }
  } else {
    Wording {
      running: "statusBar.operations.mcpServerOff.running",
      done: "statusBar.operations.mcpServerOff.done",
      failed: "statusBar.operations.mcpServerOff.failed",
    }
  };
  wording
    .named(name)
    .project(open.project.name().as_str())
    .target(in_project(open, Place::Connections))
}

/// Removing MCP server `name` from the project.
pub fn mcp_server_remove(open: &OpenProject, name: &str) -> OperationSpec {
  Wording {
    running: "statusBar.operations.mcpServerRemove.running",
    done: "statusBar.operations.mcpServerRemove.done",
    failed: "statusBar.operations.mcpServerRemove.failed",
  }
  .named(name)
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Connections))
}

/// Checking a provider's CLI in the connection dialog; named by the product, e.g. "Codex CLI".
/// `None` for a native provider, which has no CLI to check.
pub fn cli_check(open: &OpenProject, provider: AgentProvider) -> Option<OperationSpec> {
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
    AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => return None,
  };
  Some(
    wording
      .unnamed()
      .project(open.project.name().as_str())
      .target(in_project(open, Place::Connections)),
  )
}

/// Checking the GitHub CLI in the GitHub connection dialog.
pub fn gh_check(open: &OpenProject) -> OperationSpec {
  Wording {
    running: "statusBar.operations.cliCheck.gh.running",
    done: "statusBar.operations.cliCheck.gh.done",
    failed: "statusBar.operations.cliCheck.gh.failed",
  }
  .unnamed()
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Connections))
}

/// Making the project expect the account `gh` of connection `name` uses now.
pub fn expected_account(open: &OpenProject, name: &str) -> OperationSpec {
  Wording {
    running: "statusBar.operations.expectedAccount.running",
    done: "statusBar.operations.expectedAccount.done",
    failed: "statusBar.operations.expectedAccount.failed",
  }
  .named(name)
  .project(open.project.name().as_str())
  .target(in_project(open, Place::Connections))
}

/// Detecting the GitHub repository of the project folder: in setup, before the project exists,
/// or from Project settings.
pub fn repository_detect(project: Option<&OpenProject>) -> OperationSpec {
  let spec = Wording {
    running: "statusBar.operations.repositoryDetect.running",
    done: "statusBar.operations.repositoryDetect.done",
    failed: "statusBar.operations.repositoryDetect.failed",
  }
  .unnamed();
  match project {
    Some(open) => spec
      .project(open.project.name().as_str())
      .target(in_project(open, Place::Settings)),
    None => spec,
  }
}

/// Recording the detected repository `repository` (`owner/name`) for the project `project` at
/// `repo_root`: its GitHub connection and link.
pub fn repository_link(project: &str, repo_root: &Path, repository: &str) -> OperationSpec {
  Wording {
    running: "statusBar.operations.repositoryLink.running",
    done: "statusBar.operations.repositoryLink.done",
    failed: "statusBar.operations.repositoryLink.failed",
  }
  .named(repository)
  .project(project)
  .target(Target::Project {
    repo_root: repo_root.to_path_buf(),
    place: Place::Settings,
  })
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
