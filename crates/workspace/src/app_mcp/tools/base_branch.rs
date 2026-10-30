//! The base branch of tasks and modules as the tools take and give it (`orchester_core::integration::base`): kept on
//! this computer only, so in a shared project the tools set and read this computer's choice, which is never sent to
//! its server. A name the repository has only as a remote-tracking branch creates its local branch, tracking it, as
//! a person's pick of it does.

use std::path::Path;

use orchester_core::{
  integration::{
    GitError,
    base::{BaseChoice, BranchPick, BranchPickError, RunBaseSource, TaskBase, branch_choices, take_branch},
  },
  run::{BranchName, Run},
  task::Task,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use crate::{
  app_mcp::error::ToolError,
  messages::Message,
  tasks::{changes::GIT, integration::git_message},
};

/// The word for a task that takes its module's base branch.
const INHERIT: &str = "inherit";
/// The word for the branch the checkout has checked out when a run starts.
const CHECKED_OUT: &str = "checked_out";

/// A task's base branch as `given`: `inherit`, `checked_out` or a branch of the repository at `root`.
pub fn task_base(programs: &Programs, root: &Path, given: &str) -> Result<TaskBase, ToolError> {
  match given {
    INHERIT => Ok(TaskBase::Inherit),
    CHECKED_OUT => Ok(TaskBase::CheckedOut),
    named => taken(programs, root, named).map(TaskBase::Branch),
  }
}

/// A module's base branch as `given`: `checked_out` or a branch of the repository at `root`.
pub fn module_base(programs: &Programs, root: &Path, given: &str) -> Result<BaseChoice, ToolError> {
  match given {
    CHECKED_OUT => Ok(BaseChoice::CheckedOut),
    named => taken(programs, root, named).map(BaseChoice::Branch),
  }
}

/// The local branch `named`: one the repository has, else the one a remote-tracking branch of that name
/// (`release` or `origin/release`) creates; refused when the repository has neither.
fn taken(programs: &Programs, root: &Path, named: &str) -> Result<BranchName, ToolError> {
  let not_found = || {
    ToolError::refused(
      "branch_not_found",
      vec![Message::new("appMcp.errors.branchNotFound").with("branch", named.to_owned())],
    )
  };
  let branch = BranchName::new(named).map_err(|_| not_found())?;
  let choices = branch_choices(&programs.runner(), Path::new(GIT), root).map_err(|error| git_failed(&error))?;
  let remote = choices
    .remote
    .iter()
    .find(|remote| remote.local() == &branch || remote.tracking() == named);
  let pick = match remote {
    Some(remote) if !choices.has_local(&branch) => BranchPick::Remote(remote.clone()),
    _ => BranchPick::Local(branch),
  };
  take_branch(&programs.git_work(), Path::new(GIT), (root, &pick)).map_err(|error| match error {
    BranchPickError::Gone(_) => not_found(),
    BranchPickError::Git(error) => git_failed(&error),
  })
}

fn git_failed(error: &GitError) -> ToolError {
  tracing::warn!(%error, "cannot take the base branch named over MCP");
  ToolError::refused("git_failed", vec![git_message(error)])
}

/// A task's base branch as the tools give it: `choice` (inherit, checked_out or branch) with the `branch` set.
pub fn choice_json(base: &TaskBase) -> Value {
  let choice = match base {
    TaskBase::Inherit => INHERIT,
    TaskBase::CheckedOut => CHECKED_OUT,
    TaskBase::Branch(_) => "branch",
  };
  json!({ "choice": choice, "branch": base.branch().map(BranchName::as_str) })
}

/// [`choice_json`] of `task`, with the branch its next run takes as things stand and where it comes from
/// (`next_run`, as [`run_json`] names it).
pub fn task_json(db: &ProjectDb, task: &Task) -> Result<Value, ToolError> {
  let mut read = choice_json(task.base());
  let (next, source) = db.next_run_base(task)?;
  let (source, module) = source_parts(source);
  read["next_run"] = json!({
    "branch": next.branch().map(BranchName::as_str),
    "source": source,
    "module": module,
  });
  Ok(read)
}

/// A module's base branch as the tools give it: the branch, `null` for the checked-out branch.
pub fn module_json(base: &BaseChoice) -> Value {
  json!(base.branch().map(BranchName::as_str))
}

/// The base a run started from, as it recorded it: its `branch` (`null` when it took a detached HEAD's), the
/// `revision`, and where the branch came from: `source` task, module (with that `module`'s id), checked_out, or
/// project for runs recorded before tasks and modules had a base branch.
pub fn run_json(run: &Run) -> Value {
  let (source, module) = source_parts(run.base_source());
  json!({
    "branch": run.base_branch().map(BranchName::as_str),
    "revision": run.base().as_str(),
    "source": source,
    "module": module,
  })
}

fn source_parts(source: RunBaseSource) -> (&'static str, Option<String>) {
  match source {
    RunBaseSource::Task => ("task", None),
    RunBaseSource::Module { module } => ("module", Some(module.to_string())),
    RunBaseSource::CheckedOut => (CHECKED_OUT, None),
    RunBaseSource::Project => ("project", None),
  }
}
