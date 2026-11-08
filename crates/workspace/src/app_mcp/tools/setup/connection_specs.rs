//! The tool specs of agent connections: listing and adding them, API providers, models, and the accounts they run on.

use rmcp::handler::server::common::schema_for_type;

use super::{accounts, api_providers, connections};
use crate::app_mcp::tools::{Spec, across::InProjects};

pub(super) const SPECS: &[Spec] = &[
  Spec {
    name: "list_connections",
    description: "A project's connections: agent connections (id, name, provider codex_cli, claude_code_cli, \
                  anthropic_api or openai_api, program, status from the last check, CLI version, number of models, \
                  the API provider a CLI runs on: base_url, host, key_secret's name and models), decision_models \
                  (id, name, base_url, on_this_computer, key_secret id, models and last_check with answer timestamp), \
                  GitHub connections (id, name, host, status) and MCP servers, which profiles can give their \
                  agents: id, name, enabled, transport (stdio with program, args and working_directory, or http \
                  with url and authorization: its scheme none, bearer, basic with username, basic_credential or \
                  header with name, and its secret NAME@environment; the values of the url's query left out), \
                  environment (each \
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
                  version and models. A native connection is tested as its Test connection does: the project's \
                  runner reads its key and lists the endpoint's models.",
    schema: schema_for_type::<connections::InConnection>,
    read_only: false,
  },
  Spec {
    name: "list_models",
    description: "The models an agent or decision connection offers, each with the reasoning efforts it takes \
                  (none for decision models). Decision models come from the connection's last model-list check, \
                  with source decision. Agent models are offered as in the profile editor: its last check's, an \
                  API provider's or a native connection's own. An unlisted agent model can still be given to \
                  save_profile_draft (not verified); effort is kept only where listed. Agent connections include link.",
    schema: schema_for_type::<connections::InConnection>,
    read_only: true,
  },
  Spec {
    name: "list_accounts",
    description: "The person's agent CLI accounts on this computer, which every project's connections can run on \
                  (Settings › Accounts): id, cli (claude_code or codex), name (null for a Default nobody renamed), \
                  whether it is the CLI's Default (its own folder), state (signed_in, not_signed_in, \
                  sign_in_expired, sign_in_unknown, cli_not_found, folder_missing, not_checked), \
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
];
