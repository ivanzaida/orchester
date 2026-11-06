//! The loopback MCP client runs in the host relay, never in an agent's sandbox or a worktree script.

mod discovery;

pub(in crate::execution::executor::step) use self::images::save_pixels;
mod images;
mod transport;

use std::path::{Path, PathBuf};

use orchester_core::orchester_tools::{AppCall, AppDiscoveryError, ToolResult, redacted_value};
use orchester_exec::run_processes::{RunAppProcessError, RunProcesses};
use serde_json::Value;
use thiserror::Error;

use super::qa_app::StepQa;

#[derive(Debug, Error)]
enum AppError {
  #[error(transparent)]
  Process(#[from] RunAppProcessError),
  #[error(transparent)]
  Discovery(#[from] AppDiscoveryError),
  #[error("refused: no MCP discovery file for this pid was found in the user's or QA discovery folders")]
  Missing,
  #[error("refused: more than one QA discovery file names this pid")]
  Ambiguous,
  #[error("refused: cannot read the app's MCP discovery file")]
  ReadDiscovery,
  #[error(
    "refused: {} holds more than {} entries; app discovery reads only that many QA task folders",
    folder.display(),
    discovery::MAX_QA_ENTRIES
  )]
  SearchBound { folder: PathBuf },
  #[error("refused: {0}")]
  NoQaApp(String),
  #[error("refused: this executable is not an Orchester app")]
  AppName,
  #[error("refused: a QA copy is reached only when it runs from .tmp/qa/<task>/bin/ of this run's worktree")]
  QaCopyPlace,
  #[error("the app's MCP request failed")]
  Request,
  #[error("the app's MCP response is invalid or exceeds its size limit")]
  Response,
  #[error("the app's MCP server rejected the request")]
  Rejected,
  #[error("the app returned an unsupported content block; only text and PNG images can be relayed")]
  Content,
  #[error("the app returned an invalid or oversized PNG image")]
  Image,
  #[error("cannot save the app's PNG in the run's storage (.orchester/runs/<run>/screenshots/)")]
  SaveImage,
}

/// Calls `call`'s app: the invocation's QA app (`qa`) when the call leaves out its pid or names it, through its known
/// discovery file; any other app of the run through the discovery its pid has. Its PNGs are saved in the run's own
/// storage `artifacts` (ORC-114), which cleaning or reusing a workspace never touches.
pub(super) fn call(
  processes: &RunProcesses,
  (worktree, artifacts): (&Path, &Path),
  call: &AppCall,
  qa: Option<&StepQa>,
) -> ToolResult {
  match run(processes, (worktree, artifacts), call, qa) {
    Ok(result) => ToolResult::from_app(result),
    Err(error) => ToolResult::unavailable(&error.to_string()),
  }
}

/// The app's MCP `tools/call` result for `call`, reached as [`call`] reaches it, its PNGs saved and its discovery
/// token redacted, for a UI driver that reads it before it answers; why it could not be had, as the agent is told.
pub(super) fn result(
  processes: &RunProcesses,
  (worktree, artifacts): (&Path, &Path),
  call: &AppCall,
  qa: Option<&StepQa>,
) -> Result<Value, String> {
  run(processes, (worktree, artifacts), call, qa).map_err(|error| error.to_string())
}

fn run(
  processes: &RunProcesses,
  (worktree, artifacts): (&Path, &Path),
  call: &AppCall,
  qa: Option<&StepQa>,
) -> Result<Value, AppError> {
  let target = qa
    .map(|qa| qa.target(call.pid()))
    .transpose()
    .map_err(AppError::NoQaApp)?
    .flatten();
  let pid = match (&target, call.pid()) {
    (Some(target), _) => target.pid,
    (None, Some(pid)) => pid,
    (None, None) => {
      return Err(AppError::NoQaApp(
        "this step starts no QA app; give the app's pid".to_owned(),
      ));
    }
  };
  // The same boundary as stop_run_process is checked before any discovery file is read or request made.
  let process = processes.app(pid)?;
  let program = process
    .executable()
    .file_stem()
    .and_then(|name| name.to_str())
    .ok_or(AppError::AppName)?;
  if !is_app(program) {
    return Err(AppError::AppName);
  }
  let discovery = match &target {
    // Checked again at each call: a link put in its place since the app started is never followed.
    Some(target) => discovery::read(&discovery::qa_file(&target.discovery)?.ok_or(AppError::Missing)?, pid)?,
    None if is_qa_copy(program) => discovery::qa_copy(worktree, process.executable(), pid)?,
    None => discovery::find(worktree, pid)?,
  };
  let mut result = transport::Client::new(&process, &discovery)?.call(call)?;
  images::save(artifacts, &mut result)?;
  // Redact before a result can reach Debug, a relay event, the agent or persistence.
  Ok(redacted_value(result, std::slice::from_ref(discovery.token())))
}

/// Whether `program` (a file name without its extension) is the app: `orchester`, or `orchester-app` as builds before
/// the program's rename named it, or a QA copy ([`is_qa_copy`]).
fn is_app(program: &str) -> bool {
  program.eq_ignore_ascii_case("orchester") || program.eq_ignore_ascii_case("orchester-app") || is_qa_copy(program)
}

/// Whether `program` is the name `tools/qa-app.ps1` gives the copy it starts. Besides the step's QA app, such a copy is
/// reached only from `.tmp/qa/<task>/bin/` of this run's worktree, through that task folder's own discovery file
/// ([`discovery::qa_copy`]): the agent may have started it with the script itself, inside its sandbox.
fn is_qa_copy(program: &str) -> bool {
  program.eq_ignore_ascii_case("orchester-app-qa")
}
