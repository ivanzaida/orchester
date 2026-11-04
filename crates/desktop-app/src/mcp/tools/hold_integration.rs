//! `hold_integration`, in debug builds only: takes the integration lease of the project the active tab shows, as
//! another operation would, so a merge asked for meanwhile is refused at the integration lock and Orchester really tries
//! it again by itself (09j `GhnHQ`, the *Retrying* row of `Cjprb`) while QA captures it (`AGENTS.md` rule 15); then lets
//! the lease go, and the next try merges.

use std::{path::PathBuf, sync::Arc};

use lurq::mcp::{McpTool, Scope};
use orchester_exec::local_git::IntegrationLease;
use parking_lot::Mutex;
use serde_json::{Value, json};

use super::{ToolError, WindowLink, hold_reads::flag, reply};

const DESCRIPTION: &str = "Debug builds only. `held: true` takes the integration lease of the project the active tab \
  shows, as another operation would: a merge asked for meanwhile (Try again on a run's merge attempt page) is refused at \
  the integration lock, and Orchester tries it again by itself, so Retrying by itself and its countdown can be captured. \
  `held: false` lets the lease go, and Orchester's next try merges. Answers whether a lease is held, and the project \
  folder it holds.";

/// The lease the tool holds, and the folder of its project.
#[derive(Clone, Default)]
pub struct HeldLease(Arc<Mutex<Option<(PathBuf, IntegrationLease)>>>);

pub fn tool(link: WindowLink, lease: HeldLease) -> McpTool {
  McpTool::new("hold_integration")
    .description(DESCRIPTION)
    .scope(Scope::Interact)
    .input_schema(json!({
      "type": "object",
      "properties": { "held": { "type": "boolean" } },
      "required": ["held"],
      "additionalProperties": false,
    }))
    .handler(move |_ctx, args| reply(hold_integration(&link, &lease, &args)))
}

/// Takes or lets go the active project's integration lease as `args` say, and answers whether one is held and where.
pub fn hold_integration(link: &WindowLink, lease: &HeldLease, args: &Value) -> Result<Value, ToolError> {
  let held = flag(args)?;
  let mut holding = lease.0.lock();
  if !held {
    *holding = None;
  } else if holding.is_none() {
    let root = active_root(link)?;
    let taken = IntegrationLease::acquire(&root).map_err(|error| ToolError::NotSaved {
      reason: format!("the integration lease of {} can't be taken: {error}", root.display()),
    })?;
    *holding = Some((root, taken));
  }
  let folder = holding.as_ref().map(|(root, _)| root.display().to_string());
  Ok(json!({ "held": folder.is_some(), "project": folder }))
}

/// The folder of the project the active tab shows.
fn active_root(link: &WindowLink) -> Result<PathBuf, ToolError> {
  let outside = || ToolError::InvalidParam {
    param: "held",
    reason: "the active tab shows no open project".to_owned(),
  };
  let tabs = link.navigation()?.tabs_now();
  let project = tabs.active().route().project().ok_or_else(outside)?;
  link
    .projects()?
    .state_now(project)
    .and_then(|state| state.repo_root().map(PathBuf::from))
    .ok_or_else(outside)
}
