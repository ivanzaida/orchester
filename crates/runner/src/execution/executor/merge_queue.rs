//! The merge queue's work in a run's worktree (ORC-684, [`MergeQueueCommand`]): the desktop drives the queue, and the
//! runner, which reaches the worktree, syncs the base branch into it and runs Check again there on the merged result.
//! Git runs as the runner's own git work does (no console window, stopped only when it hangs); each checking tool
//! runs as its tool step runs it: found on the `PATH` or in the worktree, with the cleaned environment, the project's
//! variables (the run's storage among them) and the tool's, contained so that ending it ends everything it started,
//! and ended with all of that once the tool's timeout passes.

use std::{path::Path, time::Duration};

use orchester_core::{
  integration::delivery,
  merge_queue::recheck::{RecheckOutput, RecheckTool, Rechecked},
  resource::CommandEnvironment,
  run::CommandEnd,
};
use orchester_exec::ProcessRunner;
use orchester_ipc::{
  Rejection,
  merge_queue::{MergeQueueCommand, MergeQueueReply},
};
use orchester_store::ProjectDb;
use tokio::time::sleep;

use super::{super::Scheduler, environment::run_environment, failed};
use crate::{
  execution::coordinator::link::LocalLink,
  execution_root::ExecutionRoot,
  supervisor::{ProcessEvent, ProcessExit, Supervised},
};

/// Merging the base into a large worktree runs the repository's hooks: stopping it halfway would leave the worktree
/// half merged, so only a git that hangs is stopped, as for the runner's other git work.
const GIT_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const GIT: &str = "git";

impl Scheduler {
  /// Carries out the merge queue's `command` in a run's worktree.
  pub async fn merge_queue(&self, command: MergeQueueCommand) -> Result<MergeQueueReply, Rejection> {
    let root = self.inner.project_root.clone();
    let execution = ExecutionRoot::local(&root);
    match command {
      MergeQueueCommand::SyncBase { run, base, branch } => {
        let worktree = execution.run_worktree(run);
        let synced = tokio::task::spawn_blocking(move || {
          delivery::sync(
            &ProcessRunner::new(GIT_TIMEOUT),
            Path::new(GIT),
            (&worktree, &base, &branch),
          )
        })
        .await
        .map_err(|error| failed("syncing the run's worktree did not finish", &error))?;
        Ok(match synced {
          Ok(synced) => MergeQueueReply::Synced(synced),
          Err(error) => MergeQueueReply::SyncFailed {
            detail: error.to_string(),
          },
        })
      }
      MergeQueueCommand::Recheck { run, tools } => {
        let worktree = execution.run_worktree(run);
        // The project's variables as the coordinator gives them, made the environment of this computer; the database
        // is closed before the tools run.
        let environment = {
          let db = ProjectDb::open(&root).map_err(|error| failed("cannot open the project database", &error))?;
          run_environment((&LocalLink::over(&db), &*self.inner.computer), &execution, run)
            .map_err(|detail| Rejection::Failed { detail })?
        };
        Ok(MergeQueueReply::Rechecked(
          recheck((execution.repository(), &worktree), &tools, &environment).await,
        ))
      }
    }
  }
}

/// Runs `tools` in `worktree` of the repository at `root`, in order, until one does not pass.
async fn recheck(
  (root, worktree): (&Path, &Path),
  tools: &[RecheckTool],
  environment: &CommandEnvironment,
) -> Rechecked {
  let mut passed = Vec::new();
  for tool in tools {
    let failed = |detail: String| Rechecked::Failed {
      tool: tool.name.clone(),
      detail,
    };
    let process = match tool.process((worktree, root), environment, std::env::vars_os().collect()) {
      Ok(process) => process,
      Err(detail) => return failed(detail),
    };
    let (end, output) = match run(&process, tool.action.timeout().duration()).await {
      Ok(ended) => ended,
      Err(detail) => return failed(detail),
    };
    if let Err(detail) = tool.verdict(end, output.finish()) {
      return failed(detail);
    }
    passed.push(tool.name.clone());
  }
  Rechecked::passed(&passed)
}

/// Runs `process` contained until it ends, ending it with everything it started once `timeout` passes: how it ended
/// and what it printed; why it did not start when it did not.
async fn run(
  process: &orchester_core::command::Command,
  timeout: Duration,
) -> Result<(CommandEnd, RecheckOutput), String> {
  let mut process = Supervised::start(process).map_err(|error| error.to_string())?;
  let mut output = RecheckOutput::default();
  let deadline = sleep(timeout);
  tokio::pin!(deadline);
  let mut timed_out = false;
  let exit = loop {
    tokio::select! {
      event = process.next() => match event {
        Some(ProcessEvent::Output { stream, line }) => output.push(stream, &line),
        Some(ProcessEvent::Exited(exit)) => break exit,
        None => break ProcessExit::WaitFailed { reason: String::new() },
      },
      () = &mut deadline, if !timed_out => {
        timed_out = true;
        process.terminate();
      }
    }
  };
  let end = match (timed_out, exit) {
    (true, _) => CommandEnd::TimedOut,
    (false, ProcessExit::Exited { code }) => CommandEnd::Exited { code },
    (false, ProcessExit::Terminated | ProcessExit::WaitFailed { .. }) => CommandEnd::Exited { code: None },
  };
  Ok((end, output))
}
