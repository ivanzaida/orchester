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
//! | one of the project's published services (`start_service`) or stacks (`start_stack`, with the service tools), network on | asks | asks |
//! | stopping or looking at a service the step started (`stop_service`, `service_status`, `service_logs`), with `start_service` | runs | runs |
//! | the service tools, network off or no published service | not offered | not offered |
//! | one of the project's published test suites (`run_tests`), network on | asks at every call (no rules of their own yet) | the same |
//! | a stored test's output (`test_output`), with `run_tests` | runs | runs |
//! | the test tools, network off or no published suite | not offered | not offered |
//! | the browser tools (`browser_*`), the profile's Browser and network on | runs or asks per call, by the page's site ([`browser::decide`]) | the same |
//! | the browser tools, Browser or network off | not offered | not offered |
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

use super::{
  AccessClass, OrchesterTool, ProjectTool, SERVER, ToolCall,
  browser::{self, BrowserTool},
  catalog::input_schema,
};
use crate::{
  permission::{AgentPermissions, NetworkAccess, browser::AllowedSites},
  resource::{RepositoryAccess, Slug},
  service::{ServiceName, StackName},
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
/// every sandbox, where no host list holds, so a step without network on gets none; [`StepTools::for_step`] lets the
/// checks that ask no person run.
pub fn decide(tool: OrchesterTool, permissions: &AgentPermissions) -> Option<ToolPolicy> {
  let online = *permissions.network() == NetworkAccess::On;
  match tool.access() {
    AccessClass::ReadOnly | AccessClass::Records | AccessClass::RunProcesses => Some(ToolPolicy::Runs),
    AccessClass::WritesWorktree => (permissions.repository() == RepositoryAccess::Write).then_some(ToolPolicy::Runs),
    // A check, a service or a test suite runs a program outside every sandbox, where no host list holds.
    AccessClass::Outside
      if matches!(
        tool,
        OrchesterTool::RunCheck | OrchesterTool::StartService | OrchesterTool::StartStack | OrchesterTool::RunTests
      ) && !online =>
    {
      None
    }
    // A QA copy builds the worktree's code: a step that may not write it or reach any host gets none.
    AccessClass::Outside
      if tool == OrchesterTool::StartQaApp && (!online || permissions.repository() != RepositoryAccess::Write) =>
    {
      None
    }
    AccessClass::Outside => Some(ToolPolicy::Asks),
  }
}

/// Whether auto-approval may answer a call of `tool` of MCP server `server`: any call but one of Orchester's
/// `run_check`, `start_qa_app`, `start_service`, `start_stack` or `run_tests`, which run programs outside every
/// sandbox, or of its browser tools ([`super::browser`]), which can send anything to any site.
pub fn auto_approvable(server: &str, tool: &str) -> bool {
  server != SERVER
    || (tool != OrchesterTool::RunCheck.as_str()
      && tool != OrchesterTool::StartQaApp.as_str()
      && tool != OrchesterTool::StartService.as_str()
      && tool != OrchesterTool::StartStack.as_str()
      && tool != OrchesterTool::RunTests.as_str()
      && BrowserTool::from_name(tool).is_none())
}

/// The tools one step is offered, with their policy, the checks `run_check` runs, the services `start_service`
/// starts, the stacks `start_stack` starts, the suites `run_tests` runs, and the run's sources, which the calls outside
/// the worktree read without asking.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StepTools {
  offered: Vec<(OrchesterTool, ToolPolicy)>,
  /// Name, description and whether a call asks, of each check, in library order.
  checks: Vec<(String, String, bool)>,
  /// The names of the project's published services, when the service tools are offered ([`Self::with_services`]).
  services: Vec<String>,
  /// The names of the project's published stacks, when `start_stack` is offered ([`Self::with_stacks`]).
  stacks: Vec<String>,
  /// The slugs of the project's published test suites, when `run_tests` is offered ([`Self::with_test_suites`]).
  suites: Vec<String>,
  sources: Vec<SourcePath>,
  /// The allowed sites, the prompt names them, when the browser tools are offered ([`super::browser::offered`]).
  /// Whether a call of one asks depends on the page it acts on, so the runner decides each call itself
  /// ([`super::browser::decide`]): every CLI runs them unasked.
  browser: Option<AllowedSites>,
}

impl StepTools {
  /// The tools of a step with `permissions` offered `checks`; `run_check` only when there is one. `run_check` runs
  /// without asking when no check asks. The browser tools come with the profile's *Browser* and the step's network.
  pub fn for_step(permissions: &AgentPermissions, checks: &[ProjectTool]) -> Self {
    let all_run = checks.iter().all(|check| !check.asks());
    let offered = OrchesterTool::ALL
      .into_iter()
      .filter(|tool| *tool != OrchesterTool::RunCheck || !checks.is_empty())
      .filter(|tool| {
        !matches!(
          tool,
          OrchesterTool::FindSymbol
            | OrchesterTool::ListSkills
            | OrchesterTool::ReadSkill
            | OrchesterTool::StartService
            | OrchesterTool::StartStack
            | OrchesterTool::StopService
            | OrchesterTool::ServiceStatus
            | OrchesterTool::ServiceLogs
            | OrchesterTool::RunTests
            | OrchesterTool::TestOutput
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
      services: Vec::new(),
      stacks: Vec::new(),
      suites: Vec::new(),
      sources: Vec::new(),
      browser: browser::offered(permissions).then(|| permissions.browser().sites().clone()),
    }
  }

  /// These tools and `run_tests` with `test_output`, for a step with `permissions` of a project that publishes the test
  /// suites `suites`, after the service tools and before the run's records: none without a suite, or when the step may
  /// not run one ([`decide`]). A run asks every time, as a service start does, until suites have approval rules of
  /// their own; reading a stored test's output never asks.
  #[must_use]
  pub fn with_test_suites(mut self, permissions: &AgentPermissions, suites: &[Slug]) -> Self {
    let Some(policy) = decide(OrchesterTool::RunTests, permissions).filter(|_| !suites.is_empty()) else {
      return self;
    };
    let at = self
      .offered
      .iter()
      .position(|(tool, _)| matches!(tool, OrchesterTool::ListSkills | OrchesterTool::GetTask))
      .unwrap_or(self.offered.len());
    self.offered.splice(
      at..at,
      [
        (OrchesterTool::RunTests, policy),
        (OrchesterTool::TestOutput, ToolPolicy::Runs),
      ],
    );
    self.suites = suites.iter().map(|slug| slug.as_str().to_owned()).collect();
    self
  }

  /// Whether `suite` is one of the suites `run_tests` runs.
  pub(super) fn offers_suite(&self, suite: &Slug) -> bool {
    self.suites.iter().any(|offered| offered == suite.as_str())
  }

  /// These tools and the service tools (`start_service`, `stop_service`, `service_status`, `service_logs`), for a step
  /// with `permissions` of a project that publishes `services`: none without a service, or when the step may not start
  /// one ([`decide`]). A start asks; stopping or looking at what the step started never does.
  #[must_use]
  pub fn with_services(mut self, permissions: &AgentPermissions, services: &[ServiceName]) -> Self {
    let Some(policy) = decide(OrchesterTool::StartService, permissions).filter(|_| !services.is_empty()) else {
      return self;
    };
    // Beside the other tools that run programs, before the run's records.
    let at = self
      .offered
      .iter()
      .position(|(tool, _)| matches!(tool, OrchesterTool::ListSkills | OrchesterTool::GetTask))
      .unwrap_or(self.offered.len());
    self.offered.splice(
      at..at,
      [
        (OrchesterTool::StartService, policy),
        (OrchesterTool::StopService, ToolPolicy::Runs),
        (OrchesterTool::ServiceStatus, ToolPolicy::Runs),
        (OrchesterTool::ServiceLogs, ToolPolicy::Runs),
      ],
    );
    self.services = services.iter().map(|name| name.as_str().to_owned()).collect();
    self
  }

  /// These tools and `start_stack`, right before `start_service` with its policy, for a project that publishes
  /// `stacks`: none without a stack, or when the step is not offered the service tools ([`Self::with_services`]).
  #[must_use]
  pub fn with_stacks(mut self, stacks: &[StackName]) -> Self {
    let Some(policy) = self.policy(OrchesterTool::StartService).filter(|_| !stacks.is_empty()) else {
      return self;
    };
    let at = self
      .offered
      .iter()
      .position(|(tool, _)| *tool == OrchesterTool::StartService)
      .unwrap_or(self.offered.len());
    self.offered.insert(at, (OrchesterTool::StartStack, policy));
    self.stacks = stacks.iter().map(|name| name.as_str().to_owned()).collect();
    self
  }

  /// Whether `name` is one of the services the step's service tools take.
  pub(super) fn offers_service(&self, name: &ServiceName) -> bool {
    self.services.iter().any(|service| service == name.as_str())
  }

  /// Whether `name` is one of the stacks `start_stack` takes.
  pub(super) fn offers_stack(&self, name: &StackName) -> bool {
    self.stacks.iter().any(|stack| stack == name.as_str())
  }

  /// Whether the step is offered the browser tools.
  pub const fn offers_browser(&self) -> bool {
    self.browser.is_some()
  }

  /// The browser tools the step is offered: all of them, or none.
  fn browser_tools(&self) -> impl Iterator<Item = BrowserTool> {
    BrowserTool::ALL.into_iter().filter(|_| self.offers_browser())
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
  /// so they reach the permission prompt. Every browser tool gets one: the runner asks about a browser call itself
  /// when it arrives.
  pub fn claude_allow_rules(&self) -> Vec<String> {
    self
      .offered
      .iter()
      .filter(|(_, policy)| *policy == ToolPolicy::Runs)
      .map(|(tool, _)| tool.called(true))
      .chain(self.browser_tools().map(|tool| tool.called(true)))
      .collect()
  }

  /// The tools a Codex agent's process approves (`approval_mode = "approve"`): every one, so Codex sends no
  /// approval request of its own. The runner asks about a tool that asks when its call arrives; whether
  /// Codex 0.159.2 honours `prompt` while the step's `mcp_elicitations` is off is not verified.
  pub fn codex_approved(&self) -> Vec<&'static str> {
    self
      .offered
      .iter()
      .map(|(tool, _)| tool.as_str())
      .chain(self.browser_tools().map(BrowserTool::as_str))
      .collect()
  }

  /// The `tools` of the server's MCP `tools/list` result.
  pub fn mcp_tools(&self) -> Vec<Value> {
    let browser = self.browser_tools().map(|tool| {
      json!({
        "name": tool.as_str(),
        "description": tool.description(),
        "inputSchema": tool.input_schema(),
        "annotations": {"readOnlyHint": tool.read_only(), "destructiveHint": false, "openWorldHint": true},
      })
    });
    self
      .offered
      .iter()
      .map(|(tool, _)| {
        json!({
          "name": tool.as_str(),
          "description": tool.description(),
          "inputSchema": input_schema(*tool, &self.checks, (&self.services, &self.stacks, &self.suites)),
          "annotations": {
            "readOnlyHint": tool.read_only(),
            "destructiveHint": matches!(tool, OrchesterTool::StopRunProcess | OrchesterTool::StopService),
            "openWorldHint": tool.access() == AccessClass::Outside,
          },
        })
      })
      .chain(browser)
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
    text.push_str(&self.service_lines(claude));
    if let Some(sites) = &self.browser {
      text.push_str(&browser::prompt::lines(sites, claude));
    }
    text
  }

  /// The prompt's sentences on the service tools and `run_tests`, for the tools the step is offered.
  fn service_lines(&self, claude: bool) -> String {
    let mut text = String::new();
    if self.policy(OrchesterTool::StartService).is_some() {
      text.push_str(&format!(
        " Start the project's servers only with {}(name) ({}), never by shell: it waits until one is ready and \
         returns its url and port. See whether a server you started runs with {}(name), read its output with \
         {}(name, tail, since) and stop it with {}(name), not with shell commands such as ps, tasklist, kill or \
         taskkill. Orchester stops them when your turn ends or waits for a person.",
        OrchesterTool::StartService.called(claude),
        self.services.join(", "),
        OrchesterTool::ServiceStatus.called(claude),
        OrchesterTool::ServiceLogs.called(claude),
        OrchesterTool::StopService.called(claude),
      ));
      if self.policy(OrchesterTool::StartStack).is_some() {
        text.push_str(&format!(
          " Start a whole stack ({}) with {}(name). Both start what a service needs first.",
          self.stacks.join(", "),
          OrchesterTool::StartStack.called(claude),
        ));
      }
    }
    if self.policy(OrchesterTool::RunTests).is_some() {
      text.push_str(&format!(
        " Run the project's test suites ({}) with {}(suite, filter), never by shell: it starts what a suite needs \
         and returns its failures with their messages, not the raw output; read one test's output with {}(run, test) \
         only when that is not enough.",
        self.suites.join(", "),
        OrchesterTool::RunTests.called(claude),
        OrchesterTool::TestOutput.called(claude),
      ));
    }
    text
  }
}
