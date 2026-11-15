//! The tool specs of what a project is connected to and of the accounts its agents run on: the person's sign-ins
//! and API providers, the models each serves, and which one a profile runs on. No tool names an agent connection.

use rmcp::handler::server::common::schema_for_type;

use super::{accounts, connections, runs_on};
use crate::app_mcp::tools::{Spec, across::InProjects};

pub(super) const SPECS: &[Spec] = &[
  Spec {
    name: "list_connections",
    description: "What a project is connected to, of one project or, with no project given, of every project. \
                  runs_on has a row for each agent profile the project lists: profile {id, role}, model, status \
                  (whether a run can start on it now) and runs_on, what it runs on on this computer: agent \
                  (codex_cli, claude_code_cli, anthropic_api, openai_api) with account, one of the person's \
                  sign-ins as list_accounts gives it, or api_provider, one of their API providers; runs_on is \
                  null for a profile that runs on nothing here yet (set_profile_account). No agent connection is \
                  listed: Orchester keeps one for each account a project's profiles run on. github is the \
                  project's GitHub connection with its host, status and link; mcp_servers are its MCP servers; \
                  decision_models its decision model servers with base_url, on_this_computer, takes_key, models \
                  and last_check. No key and no secret's value is listed. Projects that could not be read are \
                  listed in unreadable_projects.",
    schema: schema_for_type::<InProjects>,
    read_only: true,
  },
  Spec {
    name: "check_connection",
    description: "Checks the project's GitHub connection again, as Check again beside the repository does, and \
                  stores what gh said: its status. Refused as not_a_github_connection for anything else: what an \
                  agent runs on is an account, checked by check_account.",
    schema: schema_for_type::<connections::InConnection>,
    read_only: false,
  },
  Spec {
    name: "list_models",
    description: "The models an account serves, each with the reasoning efforts it takes (none for decision \
                  models), as a profile's Runs on offers them: `account` is a sign-in, an API provider or a \
                  decision model's server by id or name (list_accounts); `cli` (codex or claude_code) says whose \
                  account a name is, and is needed for `default`. A sign-in's models are the ones its CLI listed                   for the project; one the project has no connection on yet lists none (checked false) until                   set_profile_account or save_profile_draft runs a profile on it. An API provider's are the ones                   the person typed (source api_provider), those of Orchester's own agent the endpoint's or the                   person's (native), a decision server's its last check's (decision). An unlisted model can                   still be given to save_profile_draft (not verified); effort is kept only where listed.",
    schema: schema_for_type::<runs_on::AccountModels>,
    read_only: true,
  },
  Spec {
    name: "list_accounts",
    description: "What agents can run on on this computer, for every project (Settings › Accounts). accounts are \
                  the person's agent CLI sign-ins: id, cli (claude_code or codex), name (null for a Default nobody \
                  renamed), whether it is the CLI's Default (its own folder), state (signed_in, not_signed_in, \
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
                  person's Check now. api_providers are the person's API providers: id, name, used_with \
                  (claude_code, codex, or orchester_agent for Orchester's own agent), host, models, key_here \
                  (whether this computer's keychain holds its key) and status (its last test's, for Orchester's \
                  own agent; null otherwise). decision_servers are their decision model servers: id, name, host, \
                  takes_key, models and whether a check listed them. programs says where each agent CLI's \
                  program is: cli, set (the path the person set, or null) and on_search_path (the copies found \
                  there, in search order). No key is listed. Adding, renaming and removing accounts and API \
                  providers, signing in and entering a key are done by a person in the window.",
    schema: schema_for_type::<accounts::NoArguments>,
    read_only: true,
  },
  Spec {
    name: "check_account",
    description: "Checks one of the person's accounts again, once for every project: `account` is a sign-in, an \
                  API provider or a decision model's server by id or name (list_accounts); `cli` (codex or \
                  claude_code) says whose account a name is, and is needed for `default`. A sign-in's CLI is \
                  asked who is signed in to its folder (read-only status commands) and the answer is kept: \
                  returns account. An API provider of Orchester's own agent is asked for its models with its key, \
                  which spends no tokens: returns api_provider with its status. An agent CLI's API provider is \
                  refused as not_tested: its CLI talks to it, and its first run tests it. A decision model's \
                  server is asked for its models (a bodyless GET {base_url}/models): returns decision_server. \
                  No key is returned.",
    schema: schema_for_type::<accounts::CheckAccount>,
    read_only: false,
  },
  Spec {
    name: "set_profile_account",
    description: "Runs an agent profile on one of the person's accounts on this computer, as choosing the account \
                  in the profile's Runs on does: `profile` is a profile of the project by id or role (a team's in a \
                  team project), `account` a sign-in or an API provider of the profile's CLI by id or name \
                  (list_accounts), `default` its Default. The project gets its connection on the account when it \
                  has none, and the profile's version stays as it is. Moving a profile that already runs on an \
                  account to another sign-in that is not the CLI's Default, or running it on an API provider it \
                  does not run on now, which sends the project's prompts and code to that provider's host, is \
                  refused as person_only unless a person turned on Let automation change agent permissions. \
                  Returns the profile and the account or api_provider.",
    schema: schema_for_type::<runs_on::SetProfileAccount>,
    read_only: false,
  },
];
