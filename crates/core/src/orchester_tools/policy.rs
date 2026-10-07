//! Which tools a step is offered and whether a call asks (`docs/interactive-tool-approvals.md`, "Orchester's
//! tools"):
//!
//! | Access class | Step that may write | Read-only step |
//! | --- | --- | --- |
//! | read-only (`read_file`, `list_files`, `search`, `find_symbol` with the code index, `git_status`, `git_diff`, `git_log`, `git_show`) | runs | runs |
//! | writes the worktree (`edit_file`, `write_file`, `git_add`, `git_commit`) | runs | not offered |
//! | outside the worktree (`read_outside_file`) | asks | asks |
//! | the project's programs (`run_project_tool`), network on | asks | asks |
//! | the project's programs, network off | not offered | not offered |
//!
//! A call that asks reaches a person as any request outside the step's permissions does, and Orchester's
//! own answers (a rule of Approve similar, auto-approval) apply to it as to an MCP call, but auto-approval
//! never runs a project's program ([`auto_approvable`]): it may reach the network, which auto-approval leaves
//! to a person for commands too. Whether the step asks before MCP actions does not change this: these tools
//! are Orchester's, not a server the profile chose. The runner holds the line itself: a call of a tool that
//! asks runs only with a person's or a rule's approval of it, given through the CLI's own prompt or asked by
//! the runner when the call arrives. Claude Code gets an allow rule for each tool that runs
//! ([`StepTools::claude_allow_rules`]), so the others reach its permission prompt; Codex runs every tool
//! without asking ([`StepTools::codex_approved`]), and the runner asks.

use serde_json::{Value, json};

use super::{AccessClass, OrchesterTool, ProjectTool, SERVER, catalog::input_schema};
use crate::{
  permission::{AgentPermissions, NetworkAccess},
  resource::RepositoryAccess,
};

/// What happens to a call of an offered tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolPolicy {
  /// It runs without asking.
  Runs,
  /// A person decides first, unless a rule of the run or auto-approval answers.
  Asks,
}

/// The policy of `tool` in a step with `permissions`; `None` when the step is not offered it. A project's
/// program runs outside every sandbox, so a step without network gets none.
pub fn decide(tool: OrchesterTool, permissions: &AgentPermissions) -> Option<ToolPolicy> {
  match tool.access() {
    AccessClass::ReadOnly => Some(ToolPolicy::Runs),
    AccessClass::WritesWorktree => (permissions.repository() == RepositoryAccess::Write).then_some(ToolPolicy::Runs),
    AccessClass::Outside if tool == OrchesterTool::RunProjectTool && permissions.network() == NetworkAccess::Off => {
      None
    }
    AccessClass::Outside => Some(ToolPolicy::Asks),
  }
}

/// Whether auto-approval may answer a call of `tool` of MCP server `server`: any call but one of Orchester's
/// `run_project_tool`.
pub fn auto_approvable(server: &str, tool: &str) -> bool {
  server != SERVER || tool != OrchesterTool::RunProjectTool.as_str()
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
      .filter(|tool| *tool != OrchesterTool::FindSymbol)
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

  /// These tools and `find_symbol`, for a step whose agent gets the code index of the run's base revision.
  #[must_use]
  pub fn with_code_index(mut self) -> Self {
    let at = self
      .offered
      .iter()
      .position(|(tool, _)| tool.file_tool().is_none())
      .unwrap_or(self.offered.len());
    self.offered.insert(at, (OrchesterTool::FindSymbol, ToolPolicy::Runs));
    self
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

  /// Whether the tool named `name` is offered and asks: a call of it runs only once approved.
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

  /// The tools a Codex agent's process approves (`approval_mode = "approve"`): every one, so Codex sends no
  /// approval request of its own. The runner asks about a tool that asks when its call arrives; whether
  /// Codex 0.159.2 honours `prompt` while the step's `mcp_elicitations` is off is not verified.
  pub fn codex_approved(&self) -> Vec<&'static str> {
    self.offered.iter().map(|(tool, _)| tool.as_str()).collect()
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

  /// The paragraph of the prompt that points the agent at the tools besides the file tools, which its *Files
  /// and commands* part names: each by the name its CLI sees, `mcp__Orchester__<tool>` for Claude Code.
  pub fn prompt_paragraph(&self, claude: bool) -> String {
    let names: Vec<String> = self
      .offered
      .iter()
      .filter(|(tool, _)| tool.file_tool().is_none())
      .map(|(tool, _)| {
        if claude {
          format!("mcp__{SERVER}__{}", tool.as_str())
        } else {
          tool.as_str().to_owned()
        }
      })
      .collect();
    format!(
      "# Orchester tools\n\nUse the shell only for builds and tests. For git and for files outside the worktree, \
       use the tools of the {SERVER} MCP server first: {}.",
      names.join(", ")
    )
  }
}
