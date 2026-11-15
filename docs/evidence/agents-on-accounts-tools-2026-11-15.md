# Agents on accounts: the MCP tools checked against the app (2026-11-15)

Step 5 of *Agents run on accounts* (`docs/plan.md`; `docs/design/agent-accounts.md`, *MCP tools*): the tools that
named agent connections are replaced by ones that name accounts. Checked on branch `module/agents-on-accounts`
against a running QA copy (`tools/qa-app.ps1 -Name accounts-final -Scenario settings -NoRunner`, the app's own
MCP server over HTTP, called with `tools/lurq-mcp-client.py`), and by the tests named below.

## Called on the running app

| Tool | Arguments | Answer |
|---|---|---|
| `list_accounts` | `{}` | `accounts`, `api_providers`, `decision_servers`, `programs`; no key, no email, no folder path |
| `check_account` | `{"account":"DeepSeek"}` (an agent CLI's API provider) | refused, code `not_tested` |
| `list_models` | `{"project":"Kontur","account":"DeepSeek"}` | `source` `api_provider`, the two models typed for it, `checked` false (the project has no connection on it) |
| `list_connections` | `{"project":"Kontur"}` | `runs_on`, `github`, `mcp_servers`, `decision_models`, `unreadable_projects`; no `agents` |
| `add_cli_connection`, `set_api_provider`, `pick_profile_connection`, `set_connection_account` | any | no such tool: the server does not serve them |

## Covered by tests

`cargo test -p orchester-app --features test-support --test mcp_domain <filter>`:

- `domain_tools_are_listed` — `set_profile_account` and `check_account` are listed, `set_profile_account` requires
  `project`, `profile`, `account`, and the four removed tools are not listed.
- `domain_accounts` — a profile moves to another sign-in only while a person lets automation widen; the same
  account again and the CLI's Default are no move; `list_connections` reports it under `runs_on`.
- `domain_api_providers` — a provider is listed with `key_here` and without its key, serves the models typed for
  it, takes a profile only while a person lets automation widen (the refusal names its host), and no result holds
  the key.
- `domain_native` — the models of Orchester's own agent's API with their efforts, a profile on it takes only an
  effort its model lists, and `check_account` answers `key_missing` when the keychain does not hold its key.
- `domain_team_set` — `set_profile_account` for a team's profile keeps the choice on this computer, gates another
  account than the effective one, never the CLI's Default, and refuses an account of another CLI.
- `domain_team_set_reviewer` — `set_reviewer_account` for a language and a decision reviewer.

`cargo test -p orchester-workspace --features test-support --test <binary> <filter>`: `decision_connections
mcp_decision` (no `key_secret`; a decision server is named by `account` in `list_models`), `orchestrator_tools
orchestrator_tools` (the chat's tools while it drives include `set_profile_account`), `team_mcp
team_mcp_readiness` (a missing connection's fix is `set_profile_account`).

## Not checked

- `check_account` on a model API Orchester's own agent calls, with its key present: it sends a real request to the
  API, which no test and no QA copy makes.
- A project set up from nothing over MCP on a machine where the CLI is not on the search path: a CLI's program is
  a person's choice in Settings › Accounts, and no tool takes one.
- The platform repository's end-to-end tests (`ivanzaida/orchester-platform`, `crates/e2e`), which build this
  repository at a pinned commit and may call the removed tools.
