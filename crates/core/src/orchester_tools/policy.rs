//! Which tools a step is offered and whether a call asks (`docs/interactive-tool-approvals.md`, "Orchester's
//! tools"):
//!
//! | Access class | Step that may write | Read-only step |
//! | --- | --- | --- |
//! | read-only (`git_status`, `git_diff`, `git_log`, `git_show`) | runs | runs |
//! | writes the worktree (`git_add`, `git_commit`) | runs | not offered |
//! | outside the worktree or the network (`read_outside_file`, `run_project_tool`) | asks | asks |
//!
//! A call that asks reaches a person as any request outside the step's permissions does, and Orchester's
//! own answers (a rule of Approve similar, auto-approval) apply to it as to an MCP call. Whether the step
//! asks before MCP actions does not change this: these tools are Orchester's, not a server the profile
//! chose. Each agent CLI is told the same table its own way: Claude Code an allow rule for each tool that
//! runs ([`StepTools::claude_allow_rules`]), Codex an approval mode for each tool
//! ([`StepTools::codex_tools`]).

use serde_json::{Map, Value, json};

use super::{AccessClass, OrchesterTool, ProjectTool, SERVER, catalog::input_schema};
use crate::{permission::AgentPermissions, resource::RepositoryAccess};

/// What happens to a call of an offered tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolPolicy {
  /// It runs without asking.
  Runs,
  /// A person decides first, unless a rule of the run or auto-approval answers.
  Asks,
}

/// The policy of `tool` in a step with `permissions`; `None` when the step is not offered it.
pub fn decide(tool: OrchesterTool, permissions: &AgentPermissions) -> Option<ToolPolicy> {
  match tool.access() {
    AccessClass::ReadOnly => Some(ToolPolicy::Runs),
    AccessClass::WritesWorktree => (permissions.repository() == RepositoryAccess::Write).then_some(ToolPolicy::Runs),
    AccessClass::Outside => Some(ToolPolicy::Asks),
  }
}

/// The tools one step is offered, with their policy, and the project tools `run_project_tool` runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StepTools {
  offered: Vec<(OrchesterTool, ToolPolicy)>,
  /// Names and descriptions of the project tools, in library order.
  projects: Vec<(String, String)>,
}

impl StepTools {
  /// The tools of a step with `permissions` that may run `projects`; `run_project_tool` only when there
  /// is one.
  pub fn for_step(permissions: &AgentPermissions, projects: &[ProjectTool]) -> Self {
    let offered = OrchesterTool::ALL
      .into_iter()
      .filter(|tool| *tool != OrchesterTool::RunProjectTool || !projects.is_empty())
      .filter_map(|tool| decide(tool, permissions).map(|policy| (tool, policy)))
      .collect();
    Self {
      offered,
      projects: projects
        .iter()
        .map(|tool| (tool.name().to_owned(), tool.description().to_owned()))
        .collect(),
    }
  }

  pub fn offered(&self) -> &[(OrchesterTool, ToolPolicy)] {
    &self.offered
  }

  pub fn policy(&self, tool: OrchesterTool) -> Option<ToolPolicy> {
    self
      .offered
      .iter()
      .find(|(offered, _)| *offered == tool)
      .map(|(_, policy)| *policy)
  }

  /// Whether the tool named `name` is offered and asks.
  pub fn asks(&self, name: &str) -> bool {
    OrchesterTool::from_name(name).and_then(|tool| self.policy(tool)) == Some(ToolPolicy::Asks)
  }

  pub(super) fn offers_project_tool(&self, name: &str) -> bool {
    self.projects.iter().any(|(project, _)| project == name)
  }

  /// One Claude Code allow rule per tool that runs, `mcp__Orchester__<tool>`; the tools that ask get none,
  /// so they reach the permission prompt.
  pub fn claude_allow_rules(&self) -> Vec<String> {
    self
      .offered
      .iter()
      .filter(|(_, policy)| *policy == ToolPolicy::Runs)
      .map(|(tool, _)| format!("mcp__{SERVER}__{}", tool.as_str()))
      .collect()
  }

  /// The server's table in a Codex thread's `mcp_servers` configuration: `approve` for each tool that runs,
  /// so Codex runs it without an approval request, and `prompt` for each tool that asks, so Codex sends one.
  pub fn codex_tools(&self) -> Value {
    let tools: Map<String, Value> = self
      .offered
      .iter()
      .map(|(tool, policy)| {
        let mode = match policy {
          ToolPolicy::Runs => "approve",
          ToolPolicy::Asks => "prompt",
        };
        (tool.as_str().to_owned(), json!({"approval_mode": mode}))
      })
      .collect();
    json!({"tools": tools})
  }

  /// The `tools` of the server's MCP `tools/list` result.
  pub fn mcp_tools(&self) -> Vec<Value> {
    self
      .offered
      .iter()
      .map(|(tool, _)| {
        json!({
          "name": tool.as_str(),
          "description": tool.description(),
          "inputSchema": input_schema(*tool, &self.projects),
          "annotations": {
            "readOnlyHint": tool.read_only(),
            "destructiveHint": false,
            "openWorldHint": tool.access() == AccessClass::Outside,
          },
        })
      })
      .collect()
  }

  /// The paragraph of the prompt that points the agent at these tools.
  pub fn prompt_paragraph(&self) -> String {
    let names: Vec<&str> = self.offered.iter().map(|(tool, _)| tool.as_str()).collect();
    format!(
      "# Tools\n\nPrefer the tools of the {SERVER} MCP server ({}) and your own tools for reading, searching and \
       editing files over shell commands: Orchester cannot tell what a shell command does, so one outside your \
       allowed commands waits for a person's approval.",
      names.join(", ")
    )
  }
}
