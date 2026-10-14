//! The loopback MCP client runs in the host relay, never in an agent's sandbox or a worktree script.

mod discovery;
mod images;
mod transport;

use std::path::{Path, PathBuf};

use orchester_core::orchester_tools::{AppCall, AppDiscoveryError, ToolResult};
use orchester_exec::run_processes::{RunAppProcessError, RunProcesses};
use thiserror::Error;

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
  #[error("refused: this executable is not an Orchester app")]
  AppName,
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
  #[error("cannot save the app's PNG under this worktree's .tmp/; the output folder must remain inside the worktree")]
  SaveImage,
}

pub(super) fn call(processes: &RunProcesses, worktree: &Path, call: &AppCall) -> ToolResult {
  match run(processes, worktree, call) {
    Ok(result) => result,
    Err(error) => ToolResult::unavailable(&error.to_string()),
  }
}

fn run(processes: &RunProcesses, worktree: &Path, call: &AppCall) -> Result<ToolResult, AppError> {
  // The same boundary as stop_run_process is checked before any discovery file is read or request made.
  let process = processes.app(call.pid())?;
  let app = process
    .executable()
    .file_stem()
    .and_then(|name| name.to_str())
    .ok_or(AppError::AppName)?;
  let app = match app {
    name if name.eq_ignore_ascii_case("orchester-app") => "orchester-app",
    name if name.eq_ignore_ascii_case("orchester") => "orchester",
    _ => return Err(AppError::AppName),
  };
  let discovery = discovery::find(worktree, call.pid(), app, call.tool())?;
  let mut result = transport::Client::new(&process, &discovery)?.call(call)?;
  images::save(worktree, &mut result)?;
  // Redact before a result can reach Debug, a relay event, the agent or persistence.
  Ok(ToolResult::from_app(result).redacted(std::slice::from_ref(discovery.token())))
}
