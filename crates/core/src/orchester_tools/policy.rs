//! Which tools a step is offered and whether a call asks (`docs/interactive-tool-approvals.md`, "Orchester's
//! tools"):
//!
//! | Access class (tools) | Step that may write the worktree | Read-only step |
//! | --- | --- | --- |
//! | read-only (`read_file`, `list_files`, `search_file`, `find_references`, `get_task`, `find_symbol` with the code index, `list_skills` and `read_skill` with skills) | runs | runs |
//! | writes the worktree (`edit_file`, `create_file`, `delete_file`, `move_file`) | runs | not offered |
//! | records with the run or the project (`save_memory`, `report_progress`) | runs | runs |
//! | outside the worktree (`search_external_file`, `read_external_file`) | asks; runs without asking in the run's sources and Rust's dependency sources | asks; runs without asking in the run's sources and Rust's dependency sources |
//! | the checks offered to the step (`run_check`), network on | runs a check whose access rules approve no call; asks for any other, once per run or at every call as they say | runs a check whose access rules approve no call; asks for any other, once per run or at every call as they say |
//! | the checks, network off | not offered | not offered |
//! | a QA copy of the app, built from the worktree (`start_qa_app`), network on | asks; Approve similar is kept to the script's content | not offered |
//! | a QA copy of the app, network off | not offered | not offered |
//!
//! A call that asks reaches a person as any request outside the step's permissions does, and Orchester's
//! own answers (a rule of Approve similar, auto-approval) apply to it as to an MCP call, but auto-approval
//! never runs a check ([`auto_approvable`]): it may reach the network, which auto-approval leaves to a person for
//! commands too. A check is a command tool of the Tools library whose access rules let the run's pipeline and the
//! step call it; one whose rules approve no call (`ApprovalMode::Never`) runs without asking, any other asks
//! ([`ProjectTool::asks`]). Whether the step asks before MCP actions does not change this: these tools
//! are Orchester's, not a server the profile chose. The runner holds the line itself: a call of a tool that
//! asks runs only with a person's or a rule's approval of it, given through the CLI's own prompt or asked by
//! the runner when the call arrives. Claude Code gets an allow rule for each tool that runs
//! ([`StepTools::claude_allow_rules`]), so the others reach its permission prompt; Codex runs every tool
//! without asking ([`StepTools::codex_approved`]), and the runner asks.

use serde_json::{Value, json};

use super::{AccessClass, OrchesterTool, ProjectTool, SERVER, ToolCall, browser::BrowserTool, catalog::input_schema};
use crate::{
  permission::{AgentPermissions, NetworkAccess},
  resource::RepositoryAccess,
  source::{RunSources, SourcePath},
};

/// What happens to a call of an offered tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolPolicy {
  /// It runs without asking.
  Runs,
  /// A person decides first, unless a rule of the run or auto-approval answers.
  Asks,
}

/// The policy of `tool` in a step with `permissions`; `None` when the step is not offered it. A check runs outside
/// every sandbox, so a step without network gets none; [`StepTools::for_step`] lets the checks that ask no person
/// run.
pub fn decide(tool: OrchesterTool, permissions: &AgentPermissions) -> Option<ToolPolicy> {
  match tool.access() {
    AccessClass::ReadOnly | AccessClass::Records | AccessClass::RunProcesses => Some(ToolPolicy::Runs),
    AccessClass::WritesWorktree => (permissions.repository() == RepositoryAccess::Write).then_some(ToolPolicy::Runs),
    AccessClass::Outside if tool == OrchesterTool::RunCheck && permissions.network() == NetworkAccess::Off => None,
    // A QA copy builds the worktree's code: a step that may not write it or reach the network gets none.
    AccessClass::Outside
      if tool == OrchesterTool::StartQaApp
        && (permissions.network() == NetworkAccess::Off || permissions.repository() != RepositoryAccess::Write) =>
    {
      None
    }
    AccessClass::Outside => Some(ToolPolicy::Asks),
  }
}

/// Whether auto-approval may answer a call of `tool` of MCP server `server`: any call but one of Orchester's
/// `run_check` or `start_qa_app`, which run programs outside every sandbox, or of its browser tools
/// ([`super::browser`]), which can send anything to any site.
pub fn auto_approvable(server: &str, tool: &str) -> bool {
  server != SERVER
    || (tool != OrchesterTool::RunCheck.as_str()
      && tool != OrchesterTool::StartQaApp.as_str()
      && BrowserTool::from_name(tool).is_none())
}

/// The tools one step is offered, with their policy, the checks `run_check` runs, and the run's sources, which the
/// calls outside the worktree read without asking.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StepTools {
  offered: Vec<(OrchesterTool, ToolPolicy)>,
  /// Name, description and whether a call asks, of each check, in library order.
  checks: Vec<(String, String, bool)>,
  sources: Vec<SourcePath>,
}

impl StepTools {
  /// The tools of a step with `permissions` offered `checks`; `run_check` only when there is one. `run_check` runs
  /// without asking when no check asks.
  pub fn for_step(permissions: &AgentPermissions, checks: &[ProjectTool]) -> Self {
    let all_run = checks.iter().all(|check| !check.asks());
    let offered = OrchesterTool::ALL
      .into_iter()
      .filter(|tool| *tool != OrchesterTool::RunCheck || !checks.is_empty())
      .filter(|tool| {
        !matches!(
          tool,
          OrchesterTool::FindSymbol | OrchesterTool::ListSkills | OrchesterTool::ReadSkill
        )
      })
      .filter_map(|tool| decide(tool, permissions).map(|policy| (tool, policy)))
      .map(|(tool, policy)| match tool {
        OrchesterTool::RunCheck if all_run => (tool, ToolPolicy::Runs),
        _ => (tool, policy),
      })
      .collect();
    Self {
      offered,
      checks: checks
        .iter()
        .map(|check| (check.name().to_owned(), check.description().to_owned(), check.asks()))
        .collect(),
      sources: Vec::new(),
    }
  }

  /// These tools in a run with `sources`, which the prompt names.
  #[must_use]
  pub fn with_sources(mut self, sources: &RunSources) -> Self {
    self.sources = sources.paths().cloned().collect();
    self
  }

  /// These tools and `find_symbol`, for a step whose agent gets the code index of the run's base revision.
  #[must_use]
  pub fn with_code_index(mut self) -> Self {
    let at = self
      .offered
      .iter()
      .position(|(tool, _)| !tool.is_file_tool())
      .unwrap_or(self.offered.len());
    self.offered.insert(at, (OrchesterTool::FindSymbol, ToolPolicy::Runs));
    self
  }

  /// These tools and `list_skills` and `read_skill`, for a step that offers its agent skills
  /// ([`super::StepSkills`]).
  #[must_use]
  pub fn with_skills(mut self) -> Self {
    self.offered.extend([
      (OrchesterTool::ListSkills, ToolPolicy::Runs),
      (OrchesterTool::ReadSkill, ToolPolicy::Runs),
    ]);
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

  /// Whether the tool named `name` is offered and asks: a call of it runs only once approved, unless
  /// [`Self::call_asks`] says the call itself does not.
  pub fn asks(&self, name: &str) -> bool {
    OrchesterTool::from_name(name).and_then(|tool| self.policy(tool)) == Some(ToolPolicy::Asks)
  }

  /// Whether `call` runs only once approved: a check by its own access rules, any other call by its tool's policy.
  pub fn call_asks(&self, call: &ToolCall) -> bool {
    match call {
      ToolCall::RunCheck { name } => self.checks.iter().any(|(check, _, asks)| check == name && *asks),
      _ => self.asks(call.tool().as_str()),
    }
  }

  pub(super) fn offers_check(&self, name: &str) -> bool {
    self.checks.iter().any(|(check, ..)| check == name)
  }

  /// One Claude Code allow rule per tool that runs, `mcp__Orchester__<tool>`; the tools that ask get none,
  /// so they reach the permission prompt.
  pub fn claude_allow_rules(&self) -> Vec<String> {
    self
      .offered
      .iter()
      .filter(|(_, policy)| *policy == ToolPolicy::Runs)
      .map(|(tool, _)| tool.called(true))
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
          "inputSchema": input_schema(*tool, &self.checks),
          "annotations": {
            "readOnlyHint": tool.read_only(),
            "destructiveHint": *tool == OrchesterTool::StopRunProcess,
            "openWorldHint": tool.access() == AccessClass::Outside,
          },
        })
      })
      .collect()
  }

  /// The paragraph of the prompt that points the agent at the tools besides the worktree's file tools, which its
  /// *Files and commands* part names: each by the name its CLI sees ([`OrchesterTool::called`]), what reaches outside
  /// the worktree and the run's sources, and how to read skills when it has any.
  pub fn prompt_paragraph(&self, claude: bool) -> String {
    let others: Vec<String> = self
      .offered
      .iter()
      .filter(|(tool, _)| !tool.is_file_tool() && !tool.reads_outside())
      .map(|(tool, _)| tool.called(claude))
      .collect();
    let mut text = format!(
      "# Orchester tools\n\nUse the shell for builds, tests and git, not to read or search files. Read and search \
       files outside the worktree with {} and {} of the {SERVER} MCP server: in the run's sources they run at once, \
       elsewhere a person is asked first.",
      OrchesterTool::ReadExternalFile.called(claude),
      OrchesterTool::SearchExternalFile.called(claude)
    );
    if !self.sources.is_empty() {
      let sources: Vec<&str> = self.sources.iter().map(SourcePath::as_str).collect();
      text.push_str(&format!(" The run's sources: {}.", sources.join(", ")));
    }
    if !others.is_empty() {
      text.push_str(&format!(" The server also has {}.", others.join(", ")));
    }
    if self.policy(OrchesterTool::ReadSkill).is_some() {
      text.push_str(&format!(
        " Read your skills with {} by name, never by path; {} lists them.",
        OrchesterTool::ReadSkill.called(claude),
        OrchesterTool::ListSkills.called(claude)
      ));
    }
    text.push_str(&format!(
      " Find apps you started with {} and stop them with {}(pid), instead of Stop-Process or kill. \
       They run without asking; executables outside this run's worktree, the runner and the agent CLI are refused. \
       Stop only apps you started. Drive that app with {}(pid, tool, arguments) instead of running tools/lurq-mcp-client.py. \
       The relay finds discovery itself, keeps its token private and returns PNG paths under .tmp/.",
      OrchesterTool::ListRunProcesses.called(claude),
      OrchesterTool::StopRunProcess.called(claude),
      OrchesterTool::CallRunApp.called(claude),
    ));
    if self.policy(OrchesterTool::StartQaApp).is_some() {
      text.push_str(&format!(
        " For a QA copy of the app, call {}(scenario) instead of running tools/qa-app.ps1: it waits while every \
         QA slot the project allows is taken, starts the copy off-screen and returns its pid; {} without pid drives \
         it, and Orchester stops it when your turn ends or waits for a person.",
        OrchesterTool::StartQaApp.called(claude),
        OrchesterTool::CallRunApp.called(claude),
      ));
    }
    text
  }
}
