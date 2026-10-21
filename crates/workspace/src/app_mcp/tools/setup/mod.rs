//! The tools that set a project up as its configuration screens do: the project itself, agent
//! connections, agent profiles, pipelines, instructions and project settings, and the end of a run's
//! work (its changes, and merging them). They are listed apart from the tools that drive work
//! ([`super::listed`]), so only the app that serves them offers them; no tool takes or returns a
//! secret's value.

mod accounts;
mod api_providers;
mod connections;
mod instructions;
mod mcp_servers;
mod merge;
pub(crate) use self::merge::merge_proposed;
mod pipeline_steps;
mod pipelines;
mod profile_input;
mod profiles;
mod project;
mod project_tools;
mod secrets;
mod skills;
mod tool_input;
mod tool_program;
pub(super) mod widening;

use rmcp::{
  handler::server::common::schema_for_type,
  model::{JsonObject, Tool},
};
use serde_json::Value;

use super::{Spec, Translate, across::InProjects, parse, tool};
use crate::app_mcp::{error::ToolError, services::Services};

const SPECS: &[Spec] = &[
  Spec {
    name: "add_project",
    description: "Adds a project in a folder that is the top of a git repository, as project setup does: Orchester \
                  keeps its data in the folder's .orchester (excluded from git) and suggests the project's key from \
                  its name. Refused for a folder that is not a git repository's top (not_a_git_repository), a \
                  repository without a commit, which runs need to branch from (no_commits), git not installed \
                  (git_not_found), or a folder that holds an Orchester project already (it is in list_projects then). Where the repository is on GitHub and a signed-in \
                  gh can read it, the project is linked to it, as setup does; a link that fails does not undo the \
                  project and is reported under `github`.",
    schema: schema_for_type::<project::AddProject>,
    read_only: false,
  },
  Spec {
    name: "get_project",
    description: "A project's setup: id, key, name, folder, limits, code index, the environment variables its agents \
                  and command tools get (plain text by design: they hold no secrets), whether agents load the \
                  repository's own agent files (AGENTS.md, CLAUDE.md), its GitHub link, how many connections, \
                  profiles, pipelines and instructions it has, and its link in the app (its Overview).",
    schema: schema_for_type::<super::projects::InProject>,
    read_only: true,
  },
  Spec {
    name: "set_project_environment",
    description: "Replaces the environment variables the project adds to every agent CLI and command tool its runs \
                  start, as Project settings › Environment does: names are letters, digits and _ (not starting with \
                  a digit), each name once ignoring case. They are stored and shown in plain text, so never put a \
                  secret in one. Adding, changing or removing any variable but RUST_LOG, RUST_BACKTRACE, CI, \
                  NO_COLOR, FORCE_COLOR, TZ, LANG, LC_ALL, LC_CTYPE, LC_COLLATE, LC_MESSAGES, LC_MONETARY, \
                  LC_NUMERIC and LC_TIME is refused as person_only unless a person turned on Let \
                  automation change agent permissions: a variable can change what programs load, where they \
                  connect, what they trust or where tools keep their configuration. An empty list removes them all. \
                  Returns the variables as saved.",
    schema: schema_for_type::<project::SetProjectEnvironment>,
    read_only: false,
  },
  Spec {
    name: "set_repository_agent_files",
    description: "Turns on or off whether the project's agents load the repository's own agent files (AGENTS.md, \
                  CLAUDE.md and the like), as Project settings › General does. Runs that start from now on use it.",
    schema: schema_for_type::<project::SetAgentFiles>,
    read_only: false,
  },
  Spec {
    name: "list_connections",
    description: "A project's connections: agent connections (id, name, provider codex_cli, claude_code_cli, \
                  anthropic_api or openai_api, program, status from the last check, CLI version, number of models, \
                  the API provider a CLI runs on: base_url, host, key_secret's name and models), \
                  GitHub connections (id, name, host, status) and MCP servers, which profiles can give their \
                  agents: id, name, enabled, transport (stdio with program, args and working_directory, or http \
                  with url and bearer_secret; the values of the url's query left out), environment (each \
                  variable's name and whether it is plain text or a secret, NAME@environment; a plain value is not \
                  listed, nor any secret's value), the tools that run without asking and the roles of the profiles \
                  whose draft or latest published version chooses it. Each connection and server has its link in \
                  the app (its inspector in Connections). Without `project`, every project's, each naming its \
                  project (unreadable_projects lists the projects that could not be read, with why).",
    schema: schema_for_type::<InProjects>,
    read_only: true,
  },
  Spec {
    name: "add_cli_connection",
    description: "Adds a connection to an agent CLI, as Connections › Add connection does: `kind` codex or claude, \
                  a unique `name`, and the CLI's `program` as an absolute path (left out: the newest copy found on \
                  the search path). A program that is not a copy found on the search path is refused as \
                  person_only, before it runs, unless a person turned on Let automation change agent permissions. \
                  The CLI is checked first with read-only commands (its version, whether it is \
                  signed in, its models); it is saved only when it answers as the expected CLI, and the check \
                  comes back either way. A CLI that is not signed in is saved and reads sign_in_required. With \
                  `api_provider` the CLI runs on that model API with an API key instead of its own sign-in: claude \
                  needs an Anthropic Messages API, codex an OpenAI Responses API; the key is an existing project \
                  secret named by id or name (no tool takes a secret's value) and the models typed are the ones \
                  profiles on it choose from. A provider is refused as person_only unless a person turned on Let \
                  automation change agent permissions: the agents would send the project's prompts, code and \
                  command output to that host.",
    schema: schema_for_type::<connections::AddCliConnection>,
    read_only: false,
  },
  Spec {
    name: "set_api_provider",
    description: "Runs a Codex or Claude Code connection on an API provider (`api_provider`: base_url, key_secret, \
                  models, as add_cli_connection takes it), or back on its CLI's own sign-in (null). Running on a \
                  host or key the connection does not run on now is refused as person_only unless a person turned \
                  on Let automation change agent permissions; going back to the sign-in, or changing only the \
                  models, is not. A connection on a provider runs on its CLI's Default account and reads no plan \
                  limits. Returns the connection as list_connections lists it.",
    schema: schema_for_type::<api_providers::SetApiProvider>,
    read_only: false,
  },
  Spec {
    name: "check_connection",
    description: "Checks a connection's program again, as its Check again does, and stores what it found: status, \
                  version and models. A native connection is not checked in this version.",
    schema: schema_for_type::<connections::InConnection>,
    read_only: false,
  },
  Spec {
    name: "list_models",
    description: "The models an agent connection's last check listed, each with the reasoning efforts it takes, as \
                  the profile editor offers them. A model it does not list can still be given to save_profile_draft \
                  (it runs as not verified); an effort is kept only where the model lists it. `link` is the \
                  connection's in the app.",
    schema: schema_for_type::<connections::InConnection>,
    read_only: true,
  },
  Spec {
    name: "list_accounts",
    description: "The person's agent CLI accounts on this computer, which every project's connections can run on \
                  (Settings › Accounts): id, cli (claude_code or codex), name (null for a Default nobody renamed), \
                  whether it is the CLI's Default (its own folder), state (signed_in, not_signed_in, \
                  sign_in_expired, sign_in_unknown, cli_not_found, folder_missing, not_supported, not_checked), \
                  whether Orchester created its folder or the person brought it, and the account it shares a \
                  sign-in with, if any, and its link in Settings › Accounts. No email and no folder are listed. \
                  plan_limits is the account's latest plan limits reading as its CLI gave it, or null when none is \
                  known: source (turn: an agent turn reported it; read: Orchester read it outside a run), \
                  observed_at (ms since the epoch) and age_seconds, state (normal, near, reached), plan, blocked \
                  (why the CLI refuses turns: window, credits, workspace, other; null when it does not), and each \
                  window by minutes (300 five-hour, 10080 weekly) with used_percent, resets_at and \
                  reset_since_reading (true when its reset has passed since: its use now is unknown, never \
                  estimated). Codex accounts are read on their own; Claude Code accounts only by a turn or the \
                  person's Check now. Adding, renaming and removing accounts \
                  and signing them in are done by a person in the window.",
    schema: schema_for_type::<accounts::NoArguments>,
    read_only: true,
  },
  Spec {
    name: "set_connection_account",
    description: "Chooses which account an agent CLI connection runs on, as the connection's Account field does: \
                  `account` is an account of the connection's CLI by id or name (list_accounts), `default` its \
                  Default. Runs that start from now on use it; a running agent keeps its account. The connection is \
                  checked again on that account. Choosing another account than the connection's is refused as \
                  person_only unless a person turned on Let automation change agent permissions: it can send the \
                  project's code to another organisation's provider.",
    schema: schema_for_type::<accounts::SetConnectionAccount>,
    read_only: false,
  },
  Spec {
    name: "add_mcp_server",
    description: "Adds an MCP server to a project, as Connections › Add MCP server does: a unique `name` (small \
                  letters, digits, - and _, starting with a letter) and either a `program` the agent's CLI starts \
                  with each agent (a name on the search path or an absolute path, stored as the fixed file it is, \
                  links followed, outside the project's repository and on this computer, as a command tool's \
                  program; with `args`, `working_directory` inside the worktree or absolute, and `environment`, where \
                  ${secret:NAME@environment} takes the value of an active project secret when an agent starts) or a \
                  remote `url` it reaches over HTTP (with an optional `bearer_secret` NAME@environment), \
                  and `exempt_tools`, the server's tools that run without asking for profiles that ask before MCP \
                  actions. A server runs only for profiles that choose it (save_profile_draft). Refused as \
                  person_only unless a person turned on Let automation change agent permissions. Returns the \
                  server as list_connections lists it.",
    schema: schema_for_type::<mcp_servers::AddMcpServer>,
    read_only: false,
  },
  Spec {
    name: "update_mcp_server",
    description: "Changes an MCP server's fields, as Edit MCP server does; a field left out keeps its value, and \
                  `program` or `url` switches how it is reached; a field the transport does not have is refused \
                  (field_not_applicable). Dropping the bearer secret and removing tools that run without asking are \
                  always taken. A new name (a run's answers approve a server's tools by its name), another program, \
                  arguments, folder, endpoint or bearer secret, any variable but RUST_LOG, RUST_BACKTRACE, CI, \
                  NO_COLOR, FORCE_COLOR, TZ, LANG and the LC_ ones (removals included), a variable that takes a \
                  secret under any name, and a tool added to exempt_tools are refused as person_only unless a person \
                  turned on Let automation change agent permissions, judged by the fields as given before anything \
                  reads the disk. Whether it is on stays.",
    schema: schema_for_type::<mcp_servers::UpdateMcpServer>,
    read_only: false,
  },
  Spec {
    name: "remove_mcp_server",
    description: "Removes an MCP server from a project. Refused as in_use while an agent profile's draft or latest \
                  published version chooses it (the reasons name their roles): switch them first, or turn the \
                  server off with set_mcp_server_enabled, which keeps its settings.",
    schema: schema_for_type::<mcp_servers::InServer>,
    read_only: false,
  },
  Spec {
    name: "set_mcp_server_enabled",
    description: "Turns an MCP server on or off for every profile that chooses it, as its inspector's Turn on and \
                  Turn off do: agents that start from now on get it, or work without it; agents at work keep what \
                  they started with. Turning off is always taken; turning on is refused as person_only unless a \
                  person turned on Let automation change agent permissions.",
    schema: schema_for_type::<mcp_servers::SetMcpServerEnabled>,
    read_only: false,
  },
  Spec {
    name: "list_profiles",
    description: "A project's agent profiles: id, role, connection, model, effort, and where each stands (draft, \
                  stale_draft or published, with its published version), and its link in the app. Pipeline steps \
                  run published profiles. Without `project`, every project's, each naming its project \
                  (unreadable_projects lists the projects that could not be read, with why).",
    schema: schema_for_type::<InProjects>,
    read_only: true,
  },
  Spec {
    name: "get_profile",
    description: "One agent profile as its editor shows it (its draft if it has one, otherwise its latest version): \
                  role, responsibilities, role instructions, connection, model, effort, permissions (repository \
                  access, extra writable folders, commands, network, approvals) and MCP servers, where it stands, \
                  which published pipeline steps run it, its revision, which save_profile_draft's \
                  expected_revision takes, and its link in the app.",
    schema: schema_for_type::<profiles::InProfile>,
    read_only: true,
  },
  Spec {
    name: "save_profile_draft",
    description: "Saves an agent profile's draft with every field of its editor; without `profile` a new profile is \
                  made. Checked as the editor checks it: a role, an agent connection of the project and a model are \
                  required; role instructions may use {{variables}}; permissions must be ones the connection's CLI \
                  enforces on this computer (repository read_only or write; commands ask_before, allowed with a \
                  list of command prefixes, or any_in_worktree; network off or on; extra writable folders only with \
                  write access); MCP servers are the project's (list_connections). Narrowing is always taken. \
                  Anything broader than the profile's latest published version (a new profile: the most \
                  restrictive permissions its CLI enforces and no MCP server) is refused as person_only unless a \
                  person turned on Let automation change agent permissions in Settings › What automation may do: write \
                  access, writable folders, broader commands, network on, MCP servers, no longer asking before \
                  tool calls, approving automatically (which also needs Let automation answer approvals). Left \
                  out, a new profile gets the most restrictive permissions. With expected_revision, the revision \
                  get_profile or the last save returned, the draft is saved only while it is unchanged, else \
                  refused as draft_changed; without it the save goes over a change a person or another agent \
                  saved meanwhile. Publish it with publish_profile.",
    schema: schema_for_type::<profile_input::SaveProfileDraft>,
    read_only: false,
  },
  Spec {
    name: "publish_profile",
    description: "Publishes an agent profile's draft as its next version, as Publish does, so pipeline steps can run \
                  it. Refused for a draft started from an older version than the latest, a connection that is gone \
                  or does not run agents, permissions its CLI cannot enforce, as person_only a draft broader than \
                  the latest version while Let automation change agent permissions is off, and as draft_changed a \
                  draft that changed while it was checked.",
    schema: schema_for_type::<profiles::InProfile>,
    read_only: false,
  },
  Spec {
    name: "get_pipeline",
    description: "One pipeline as its editor shows it (its draft if it has one, otherwise its latest version): \
                  description and steps (key, name, agent profile or tool, runs after, approval gate, failure route \
                  with retries, step budget, repository access, model override and narrowed permissions, \
                  instructions), where it stands, the problems that keep it from being published, and the published \
                  profiles and tools its steps can use, and its link in the app (its editor).",
    schema: schema_for_type::<pipelines::InPipeline>,
    read_only: true,
  },
  Spec {
    name: "create_pipeline",
    description: "Makes a new pipeline as draft v1, as New pipeline does: `from` starter (Feature delivery: \
                  Research, Plan with an approval gate, Implement, Run checks and Review returning to Implement, Create \
                  pull request; steps get the published profiles and tools named Explorer, Planner, Builder, \
                  Reviewer, Run test suite and Create pull request where exactly one has the name), empty, or copy \
                  (`copy_of` names a pipeline whose draft or latest version is copied). The name must give a slug no \
                  other pipeline has.",
    schema: schema_for_type::<pipelines::CreatePipeline>,
    read_only: false,
  },
  Spec {
    name: "save_pipeline_draft",
    description: "Replaces a pipeline's draft with the steps given, as its editor saves it. Each step has a key \
                  (small letters, digits and -), a name, and an agent profile or a tool (left out: unassigned, which \
                  blocks publishing only). An agent step may set repository access (write only where its profile \
                  writes), a model override, and narrow its profile's permissions: commands to some of the profile's \
                  allowed list, network off, or asking about tool calls; never more than the profile allows. An agent \
                  step may also say whether its agent merges the base branch when it moves (takes_base_moves); left \
                  out, a step with write access does unless it reviews or verifies (review or verify in its key, or \
                  a failure route back to a writing step). A step \
                  runs after the steps it names (no cycles), may wait at an approval gate, may return to an earlier \
                  step it runs after when it fails (1 or more retries), and may have a token budget (left out: no \
                  step budget). Refused with the editor's reasons when the steps do not form a pipeline; nothing is \
                  saved then. A command step names a published command tool (publish_tool) as `tool`, runs after \
                  the steps it checks, and may return to one of them when the program fails, e.g. {\"key\": \
                  \"check\", \"name\": \"Check\", \"tool\": \"Check\", \"runs_after\": [\"implement\"], \
                  \"on_failure\": {\"return_to\": \"implement\", \"retries\": 2}}. Unless a person turned on Let \
                  automation change agent permissions, these are refused as person_only: dropping an approval \
                  gate, or asking about tool calls, that the latest published version (before the first: the \
                  draft) has, a step removed with it included; and a step that calls a tool neither the draft nor \
                  the latest version calls under its key, since it runs a program on this computer.",
    schema: schema_for_type::<pipeline_steps::SavePipelineDraft>,
    read_only: false,
  },
  Spec {
    name: "publish_pipeline",
    description: "Publishes a pipeline's draft as its next version, as Publish does, so tasks can run it. Refused \
                  while it has no steps or a step has no profile or tool, or when the draft was started from an older \
                  version than the latest. Unless a person turned on Let automation change agent permissions, only a \
                  draft provably no weaker than the latest published version is published (the same steps, wiring, \
                  failure routes, profiles and tools, each step's approval, asking about tool calls, write access, \
                  commands and network the same or stricter); a pipeline's first version, and anything else, is \
                  refused as person_only. So every version a task can run is one a person published, allowed, or \
                  could not object to.",
    schema: schema_for_type::<pipelines::InPipeline>,
    read_only: false,
  },
  Spec {
    name: "list_instructions",
    description: "A project's instructions, which Orchester adds to agents' prompts: id, name, where each applies \
                  (project, agent_role, pipeline or step), which agents it reaches (all, roles or profiles), when \
                  (start, resume), its text, where it stands (draft, stale_draft or published), and its link in \
                  the app (Instructions, which opens on its library). Without `project`, every project's, each \
                  naming its project (unreadable_projects lists the projects that could not be read, with why).",
    schema: schema_for_type::<InProjects>,
    read_only: true,
  },
  Spec {
    name: "get_instruction",
    description: "One instruction as its editor shows it (its draft if it has one, otherwise its latest version): \
                  its text, where it applies (project, agent_role, pipeline or step), which agents it reaches and \
                  when, where it stands, every published version with when it was published and its text, and its \
                  link in the app.",
    schema: schema_for_type::<instructions::InInstruction>,
    read_only: true,
  },
  Spec {
    name: "save_instruction_draft",
    description: "Saves an instruction's draft as its editor does; without `instruction` a new one is made under \
                  `name`. `applies_in` is project, agent_role (with `role`; it then reaches every agent of the \
                  role), pipeline (with `pipeline`) or step (with `pipeline` and `step`); `agents` all, or some \
                  roles or profiles; `events` start, resume or both. The text may use {{variables}} and may be blank \
                  in a draft. Publish it with publish_instruction.",
    schema: schema_for_type::<instructions::SaveInstructionDraft>,
    read_only: false,
  },
  Spec {
    name: "publish_instruction",
    description: "Publishes an instruction's draft as its next version, as Publish does: agents get it from then on. \
                  Refused for a blank text or a draft started from an older version than the latest.",
    schema: schema_for_type::<instructions::InInstruction>,
    read_only: false,
  },
  Spec {
    name: "get_changes",
    description: "What a run changed, as the task's Changes tab reads it: the files of its last checkpoint against \
                  the base it started from (path, change, additions, deletions), and what its worktree holds that \
                  no checkpoint has yet. Diffs too large to read give their size only. `link` is its task's \
                  Changes tab in the app, which shows the task's latest run.",
    schema: schema_for_type::<super::runs::InRun>,
    read_only: true,
  },
  Spec {
    name: "merge_run",
    description: "Merges a run, its task's latest that succeeded, into its base branch and waits for the merge, \
                  when the main checkout needs no local integration: nothing local in the way and no local \
                  integration of the run under way. Otherwise it refuses (needs_decision, merge_failed): preparing \
                  the checkout, conflict choices and approving the exact merged result are a person's, in the \
                  app's Local integration page. A run merged already, by Orchester or outside it as git proves \
                  (its base branch holds the run's commits), is refused with already_merged.",
    schema: schema_for_type::<super::runs::InRun>,
    read_only: false,
  },
];

/// Every setup tool, as `tools/list` answers.
pub fn listed() -> Vec<Tool> {
  SPECS
    .iter()
    .chain(project_tools::SPECS)
    .chain(skills::SPECS)
    .chain(secrets::SPECS)
    .map(tool)
    .collect()
}

/// Calls setup tool `name` with `arguments`; `None` when there is no such tool.
pub async fn call(
  services: &Services,
  translate: &Translate,
  name: &str,
  arguments: JsonObject,
) -> Option<Result<Value, ToolError>> {
  let named = |specs: &[Spec]| specs.iter().any(|spec| spec.name == name);
  if named(project_tools::SPECS) {
    return Some(project_tools::call(services, translate, name, Value::Object(arguments)).await);
  }
  if named(skills::SPECS) {
    return Some(skills::call(services, translate, name, Value::Object(arguments)).await);
  }
  if named(secrets::SPECS) {
    return Some(secrets::call(services, translate, name, Value::Object(arguments)).await);
  }
  if !SPECS.iter().any(|spec| spec.name == name) {
    return None;
  }
  Some(dispatch(services, translate, name, Value::Object(arguments)).await)
}

async fn dispatch(
  services: &Services,
  translate: &Translate,
  name: &str,
  arguments: Value,
) -> Result<Value, ToolError> {
  match name {
    "add_project" => project::add_project(services, parse(arguments)?, translate).await,
    "get_project" => project::get_project(services, parse(arguments)?).await,
    "set_project_environment" => project::set_environment(services, parse(arguments)?).await,
    "set_repository_agent_files" => project::set_agent_files(services, parse(arguments)?).await,
    "list_connections" => connections::list_connections(services, parse(arguments)?, translate).await,
    "add_cli_connection" => connections::add_cli_connection(services, parse(arguments)?).await,
    "check_connection" => connections::check_connection(services, parse(arguments)?).await,
    "list_models" => connections::list_models(services, parse(arguments)?).await,
    "set_api_provider" => api_providers::set_api_provider(services, parse(arguments)?).await,
    "list_accounts" => accounts::list_accounts(services, parse(arguments)?).await,
    "set_connection_account" => accounts::set_connection_account(services, parse(arguments)?).await,
    "add_mcp_server" => mcp_servers::add_mcp_server(services, parse(arguments)?).await,
    "update_mcp_server" => mcp_servers::update_mcp_server(services, parse(arguments)?).await,
    "remove_mcp_server" => mcp_servers::remove_mcp_server(services, parse(arguments)?).await,
    "set_mcp_server_enabled" => mcp_servers::set_mcp_server_enabled(services, parse(arguments)?).await,
    other => configure(services, translate, other, arguments).await,
  }
}

/// The tools of profiles, pipelines, instructions and a run's changes.
async fn configure(
  services: &Services,
  translate: &Translate,
  name: &str,
  arguments: Value,
) -> Result<Value, ToolError> {
  match name {
    "list_profiles" => profiles::list_profiles(services, parse(arguments)?, translate).await,
    "get_profile" => profiles::get_profile(services, parse(arguments)?).await,
    "save_profile_draft" => profile_input::save_profile_draft(services, parse(arguments)?).await,
    "publish_profile" => profiles::publish_profile(services, parse(arguments)?).await,
    "get_pipeline" => pipelines::get_pipeline(services, parse(arguments)?, translate).await,
    "create_pipeline" => pipelines::create_pipeline(services, parse(arguments)?, translate).await,
    "save_pipeline_draft" => pipeline_steps::save_pipeline_draft(services, parse(arguments)?, translate).await,
    "publish_pipeline" => pipelines::publish_pipeline(services, parse(arguments)?, translate).await,
    "list_instructions" => instructions::list_instructions(services, parse(arguments)?, translate).await,
    "get_instruction" => instructions::get_instruction(services, parse(arguments)?).await,
    "save_instruction_draft" => instructions::save_instruction_draft(services, parse(arguments)?).await,
    "publish_instruction" => instructions::publish_instruction(services, parse(arguments)?).await,
    "get_changes" => merge::get_changes(services, parse(arguments)?).await,
    "merge_run" => merge::merge_run(services, parse(arguments)?, translate).await,
    other => Err(ToolError::InvalidArguments(format!("no tool is named {other}"))),
  }
}
