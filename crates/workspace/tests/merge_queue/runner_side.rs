//! The project's runner's side of the merge queue's work in a run's worktree (ORC-684), here in the test's process:
//! the base is synced in with git as a process, and each checking tool is found and judged as the runner does
//! (`orchester_core::merge_queue::recheck`), its program run by the test's runner, which answers for the check. Every
//! command and reply crosses as JSON, as it does over IPC.

use std::{path::PathBuf, sync::Arc, time::Duration};

use orchester_core::{
  command::CommandRunner,
  id::RunId,
  integration::delivery,
  merge_queue::recheck::{RecheckOutput, RecheckTool, Rechecked},
  run::CommandEnd,
  store::SettingsStore,
};
use orchester_exec::ProcessRunner;
use orchester_ipc::merge_queue::{MergeQueueCommand, MergeQueueReply};
use orchester_store::ProjectDb;
use orchester_workspace::{merge_queue::RunWorktrees, messages::Message};
use serde::{Serialize, de::DeserializeOwned};

/// The runs' worktrees of the project at `root`, as its runner reaches them.
pub struct RunnerSide {
  pub root: PathBuf,
  /// Runs the checks' programs.
  pub programs: Arc<dyn CommandRunner + Send + Sync>,
}

impl RunWorktrees for RunnerSide {
  fn carry_out(&self, command: MergeQueueCommand) -> Result<MergeQueueReply, Message> {
    let reply = match crossed(&command) {
      MergeQueueCommand::SyncBase { run, base, branch } => {
        let git = ProcessRunner::new(Duration::from_secs(120));
        match delivery::sync(&git, "git".as_ref(), (&self.worktree(run), &base, &branch)) {
          Ok(synced) => MergeQueueReply::Synced(synced),
          Err(error) => MergeQueueReply::SyncFailed {
            detail: error.to_string(),
          },
        }
      }
      MergeQueueCommand::Recheck { run, tools } => MergeQueueReply::Rechecked(self.recheck(run, &tools)),
    };
    Ok(crossed(&reply))
  }
}

impl RunnerSide {
  fn worktree(&self, run: RunId) -> PathBuf {
    self.root.join(".orchester/worktrees").join(run.to_string())
  }

  fn recheck(&self, run: RunId, tools: &[RecheckTool]) -> Rechecked {
    let environment = ProjectDb::open(&self.root)
      .expect("open")
      .project_environment()
      .expect("environment");
    let worktree = self.worktree(run);
    let mut passed = Vec::new();
    for tool in tools {
      let failed = |detail: String| Rechecked::Failed {
        tool: tool.name.clone(),
        detail,
      };
      let process = match tool.process((&worktree, &self.root), &environment, std::env::vars_os().collect()) {
        Ok(process) => process,
        Err(detail) => return failed(detail),
      };
      let (end, output) = match self.programs.run(&process) {
        Ok(output) => (
          CommandEnd::Exited { code: output.code },
          RecheckOutput::of(&output.stdout, &output.stderr),
        ),
        Err(error) => return failed(error.to_string()),
      };
      if let Err(detail) = tool.verdict(end, output.finish()) {
        return failed(detail);
      }
      passed.push(tool.name.clone());
    }
    Rechecked::passed(&passed)
  }
}

/// `value` as the other side reads it after it crossed as JSON.
fn crossed<T: Serialize + DeserializeOwned>(value: &T) -> T {
  serde_json::from_str(&serde_json::to_string(value).expect("serialize")).expect("deserialize")
}
