//! The tools that set a project up as its configuration screens do: the project itself, agent
//! connections, agent profiles, pipelines, instructions and project settings, and the end of a run's
//! work (its changes, and merging them). They are listed apart from the tools that drive work
//! ([`super::listed`]), so only the app that serves them offers them; no tool takes or returns a
//! secret's value.

mod connections;
mod instructions;
mod merge;
mod pipeline_steps;
mod pipelines;
mod profile_input;
mod profiles;
mod project;
mod widening;

use rmcp::{
  handler::server::common::schema_for_type,
  model::{JsonObject, Tool},
};
use serde_json::Value;

use super::{Spec, Translate, parse, tool};
use crate::app_mcp::{error::ToolError, services::Services};

const SPECS: &[Spec] = &[
  Spec {
    name: "add_project",
    description: "Adds a project in a folder that is the top of a git repository, as project setup does: Orchester \
                  keeps its data in the folder's .orchester (excluded from git) and suggests the project's key from \
                  its name. Refused for a folder that is not a git repository's top, or that holds an Orchester \
                  project already (it is in list_projects then). Where the repository is on GitHub and a signed-in \
                  gh can read it, the project is linked to it, as setup does; a link that fails does not undo the \
                  project and is reported under `github`.",
    schema: schema_for_type::<project::AddProject>,
    read_only: false,
  },
  Spec {
    name: "get_project",
    description: "A project's setup: id, key, name, folder, limits, code index, the environment variables its agents \
                  and command tools get (plain text by design: they hold no secrets), whether agents load the \
                  repository's own agent files (AGENTS.md, CLAUDE.md), its GitHub link, and how many connections, \
                  profiles, pipelines and instructions it has.",
    schema: schema_for_type::<super::projects::InProject>,
    read_only: true,
  },
  Spec {
    name: "set_project_environment",
    description: "Replaces the environment variables the project adds to every agent CLI and command tool its runs \
                  start, as Project settings › Environment does: names are letters, digits and _ (not starting with \
                  a digit), each name once ignoring case. They are stored and shown in plain text, so never put a \
                  secret in one. Setting a variable that changes what programs load, where they connect or where \
                  tools keep their configuration (PATH, NODE_OPTIONS, *_BASE_URL, *_PROXY, *_HOME, LD_*, DYLD_*, \
                  GIT_CONFIG*, …) is refused as person_only unless a person turned on Let automation change agent \
                  permissions; removing one is not. An empty list removes them all. Returns the variables as saved.",
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
                  anthropic_api or openai_api, program, status from the last check, CLI version, number of models), \
                  GitHub connections (id, name, host, status) and MCP servers (id, name), which profiles can give \
                  their agents. Secrets and their values are never listed.",
    schema: schema_for_type::<super::projects::InProject>,
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
                  comes back either way. A CLI that is not signed in is saved and reads sign_in_required.",
    schema: schema_for_type::<connections::AddCliConnection>,
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
                  (it runs as not verified); an effort is kept only where the model lists it.",
    schema: schema_for_type::<connections::InConnection>,
    read_only: true,
  },
  Spec {
    name: "list_profiles",
    description: "A project's agent profiles: id, role, connection, model, effort, and where each stands (draft, \
                  stale_draft or published, with its published version). Pipeline steps run published profiles.",
    schema: schema_for_type::<super::projects::InProject>,
    read_only: true,
  },
  Spec {
    name: "get_profile",
    description: "One agent profile as its editor shows it (its draft if it has one, otherwise its latest version): \
                  role, responsibilities, role instructions, connection, model, effort, permissions (repository \
                  access, extra writable folders, commands, network, approvals) and MCP servers, where it stands, \
                  and which published pipeline steps run it.",
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
                  tool calls, approving automatically. Left out, a new profile gets the most restrictive \
                  permissions. Publish it with publish_profile.",
    schema: schema_for_type::<profile_input::SaveProfileDraft>,
    read_only: false,
  },
  Spec {
    name: "publish_profile",
    description: "Publishes an agent profile's draft as its next version, as Publish does, so pipeline steps can run \
                  it. Refused for a draft started from an older version than the latest, a connection that is gone \
                  or does not run agents, permissions its CLI cannot enforce, or, as person_only, a draft broader \
                  than the latest version while Let automation change agent permissions is off.",
    schema: schema_for_type::<profiles::InProfile>,
    read_only: false,
  },
  Spec {
    name: "get_pipeline",
    description: "One pipeline as its editor shows it (its draft if it has one, otherwise its latest version): \
                  description and steps (key, name, agent profile or tool, runs after, approval gate, failure route \
                  with retries, step budget, repository access, model override and narrowed permissions, \
                  instructions), where it stands, the problems that keep it from being published, and the published \
                  profiles and tools its steps can use.",
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
                  allowed list, network off, or asking about tool calls; never more than the profile allows. A step \
                  runs after the steps it names (no cycles), may wait at an approval gate, may return to an earlier \
                  step it runs after when it fails (1 or more retries), and may have a token budget (left out: no \
                  step budget). Refused with the editor's reasons when the steps do not form a pipeline; nothing is \
                  saved then.",
    schema: schema_for_type::<pipeline_steps::SavePipelineDraft>,
    read_only: false,
  },
  Spec {
    name: "publish_pipeline",
    description: "Publishes a pipeline's draft as its next version, as Publish does, so tasks can run it. Refused \
                  while it has no steps or a step has no profile or tool, or when the draft was started from an older \
                  version than the latest.",
    schema: schema_for_type::<pipelines::InPipeline>,
    read_only: false,
  },
  Spec {
    name: "list_instructions",
    description: "A project's instructions, which Orchester adds to agents' prompts: id, name, where each applies \
                  (project, agent_role, pipeline or step), which agents it reaches (all, roles or profiles), when \
                  (start, resume), its text, and where it stands (draft, stale_draft or published).",
    schema: schema_for_type::<super::projects::InProject>,
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
                  no checkpoint has yet. Diffs too large to read give their size only.",
    schema: schema_for_type::<super::runs::InRun>,
    read_only: true,
  },
  Spec {
    name: "merge_run",
    description: "Merges a run that is ready for review into the branch it started from, as the task Summary's Merge \
                  into main does, then removes its worktrees and merged branches that hold nothing more. Waits for \
                  the merge and answers how it went (fast_forward, merge_commit or already_merged). Refused unless \
                  the run succeeded and was not merged, while a merge or worktree removal of its task runs, or with \
                  git's reason (conflicts, uncommitted changes on the base branch, another branch checked out).",
    schema: schema_for_type::<super::runs::InRun>,
    read_only: false,
  },
];

/// Every setup tool, as `tools/list` answers.
pub fn listed() -> Vec<Tool> {
  SPECS.iter().map(tool).collect()
}

/// Calls setup tool `name` with `arguments`; `None` when there is no such tool.
pub async fn call(
  services: &Services,
  translate: &Translate,
  name: &str,
  arguments: JsonObject,
) -> Option<Result<Value, ToolError>> {
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
    "list_connections" => connections::list_connections(services, parse(arguments)?).await,
    "add_cli_connection" => connections::add_cli_connection(services, parse(arguments)?).await,
    "check_connection" => connections::check_connection(services, parse(arguments)?).await,
    "list_models" => connections::list_models(services, parse(arguments)?).await,
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
    "list_profiles" => profiles::list_profiles(services, parse(arguments)?).await,
    "get_profile" => profiles::get_profile(services, parse(arguments)?).await,
    "save_profile_draft" => profile_input::save_profile_draft(services, parse(arguments)?).await,
    "publish_profile" => profiles::publish_profile(services, parse(arguments)?).await,
    "get_pipeline" => pipelines::get_pipeline(services, parse(arguments)?, translate).await,
    "create_pipeline" => pipelines::create_pipeline(services, parse(arguments)?, translate).await,
    "save_pipeline_draft" => pipeline_steps::save_pipeline_draft(services, parse(arguments)?, translate).await,
    "publish_pipeline" => pipelines::publish_pipeline(services, parse(arguments)?, translate).await,
    "list_instructions" => instructions::list_instructions(services, parse(arguments)?).await,
    "save_instruction_draft" => instructions::save_instruction_draft(services, parse(arguments)?).await,
    "publish_instruction" => instructions::publish_instruction(services, parse(arguments)?).await,
    "get_changes" => merge::get_changes(services, parse(arguments)?).await,
    "merge_run" => merge::merge_run(services, parse(arguments)?, translate).await,
    other => Err(ToolError::InvalidArguments(format!("no tool is named {other}"))),
  }
}
