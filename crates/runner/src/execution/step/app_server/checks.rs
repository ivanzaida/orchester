//! What the runner checks of Codex App Server's answers before the turn starts: the thread it started or
//! resumed (sandbox, approval policy, reviewer, network, writable roots, model and local environment), and
//! the MCP servers of its inventory.

use std::{
  collections::BTreeSet,
  path::{Path, PathBuf},
};

use orchester_core::{
  connection::{
    app_server::Update,
    app_server_mcp::{unavailable_selected, verify_only_chosen_run, verify_selected_servers},
    mcp::McpServerProblem,
    stream::AgentEvent,
  },
  permission::NetworkAccess,
  resource::RepositoryAccess,
};
use serde_json::Value;

use super::AppServerSession;

impl AppServerSession {
  pub(super) fn check_thread(&self, result: &Value) -> Result<(), String> {
    if let Some(expected) = &self.setup.resume
      && result.pointer("/thread/id").and_then(Value::as_str) != Some(expected.as_str())
    {
      return Err("Codex resumed a different session".to_owned());
    }
    self.check_environment(result)?;
    let granular = result
      .get("approvalPolicy")
      .and_then(|policy| policy.get("granular"))
      .ok_or_else(|| "Codex did not activate granular tool approval policy".to_owned())?;
    let enabled = |key: &str| granular.get(key).and_then(Value::as_bool).unwrap_or(false);
    if enabled("mcp_elicitations") != self.setup.tool_approvals
      || enabled("rules") != self.setup.command_approvals
      || enabled("sandbox_approval") != self.setup.sandbox_approvals
      || enabled("request_permissions")
      || enabled("skill_approval")
    {
      return Err("Codex activated a broader approval policy than this step permits".to_owned());
    }
    if result.get("approvalsReviewer").and_then(Value::as_str) != Some("user") {
      return Err("Codex did not route tool approvals to the user".to_owned());
    }
    let sandbox = result
      .get("sandbox")
      .and_then(|sandbox| sandbox.get("type"))
      .and_then(Value::as_str);
    let expected = match self.setup.access {
      RepositoryAccess::ReadOnly => "readOnly",
      RepositoryAccess::Write => "workspaceWrite",
    };
    if sandbox != Some(expected) {
      return Err("Codex activated an unexpected sandbox".to_owned());
    }
    let network = result
      .get("sandbox")
      .and_then(|sandbox| sandbox.get("networkAccess"))
      .and_then(Value::as_bool)
      .unwrap_or(false);
    if network != (self.setup.access == RepositoryAccess::Write && self.setup.network == NetworkAccess::On) {
      return Err("Codex activated unexpected network access".to_owned());
    }
    if self.setup.access == RepositoryAccess::Write {
      let roots = result
        .get("sandbox")
        .and_then(|sandbox| sandbox.get("writableRoots"))
        .and_then(Value::as_array)
        .ok_or_else(|| "Codex omitted writable sandbox roots".to_owned())?;
      let actual: Option<BTreeSet<_>> = roots.iter().map(|root| root.as_str().map(PathBuf::from)).collect();
      let mut actual = actual.ok_or_else(|| "Codex reported invalid writable sandbox roots".to_owned())?;
      // Codex reports the local cwd as an environment workspace root, not an extra sandbox root.
      actual.insert(self.setup.worktree.clone());
      let expected: BTreeSet<PathBuf> = self.setup.writable_roots.iter().cloned().collect();
      let mut allowed = expected;
      allowed.insert(self.setup.worktree.clone());
      if actual != allowed {
        return Err("Codex writable roots do not match this step's grants".to_owned());
      }
    }
    if result.get("model").and_then(Value::as_str) != Some(&self.setup.model) {
      return Err("Codex selected a different model for the tool-approval step".to_owned());
    }
    if let Some(provider) = &self.setup.model_provider
      && result.get("modelProvider").and_then(Value::as_str) != Some(provider)
    {
      return Err("Codex selected another model provider than the connection's API provider".to_owned());
    }
    Ok(())
  }

  fn check_environment(&self, result: &Value) -> Result<(), String> {
    let environments = result
      .pointer("/thread/environments")
      .and_then(Value::as_array)
      .ok_or_else(|| "Codex did not expose the local environment".to_owned())?;
    let [local] = environments.as_slice() else {
      return Err("Codex did not select exactly one local environment".to_owned());
    };
    let roots: Option<BTreeSet<_>> = local
      .get("runtimeWorkspaceRoots")
      .and_then(Value::as_array)
      .and_then(|roots| roots.iter().map(|root| root.as_str().map(PathBuf::from)).collect());
    if local.get("environmentId").and_then(Value::as_str) != Some("local")
      || local.get("cwd").and_then(Value::as_str).map(Path::new) != Some(self.setup.worktree.as_path())
      || roots != Some(self.setup.runtime_roots())
    {
      return Err("Codex selected an environment outside this worktree".to_owned());
    }
    Ok(())
  }

  /// The selected MCP servers the agent works without, as activity, each with its state and Codex's
  /// reason; an error naming the server and the reason when the step needs them all (it asks before MCP
  /// actions), or when the inventory cannot be read.
  pub(super) fn check_servers(&self, result: &Value) -> Result<Vec<Update>, String> {
    verify_only_chosen_run(result, &self.setup.expected_servers).map_err(|error| error.to_string())?;
    if self.setup.tool_approvals {
      verify_selected_servers(result, &self.setup.expected_servers, |text| self.one_line(text))
        .map_err(|error| error.to_string())?;
      return Ok(Vec::new());
    }
    let unavailable = unavailable_selected(result, &self.setup.expected_servers, |text| self.one_line(text))
      .map_err(|error| error.to_string())?;
    Ok(
      unavailable
        .into_iter()
        .map(|server| {
          let detail = server.detail();
          Update::Activity(AgentEvent::McpServer {
            server: Some(server.name),
            problem: McpServerProblem::FailedToStart { detail },
          })
        })
        .collect(),
    )
  }
}
