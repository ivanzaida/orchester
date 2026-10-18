# API providers: agent CLIs on third-party model APIs

Asked 2026-10-18: *the ability to add agents from DeepSeek etc., through an API key, with an Anthropic/Codex
interface.* This note is the design of what was built on branch `claude/api-providers`. The person lifted decision 7 of
`docs/architecture/native-agent.md` (*no custom base URL or key for CLI connections*) on 2026-10-18: API providers
on the CLI connections go first; the native agent plan stays as it is. The native agent (Orchester's own loop on a model API) is unchanged and separate.

The design is in `data/orchester2.pen`, Connections column `mKKPM`: `virfc`, `UW2bO`, `VvWpe`, `FsIt3`, `y2qhJ`,
`tnEKp`, `b3aoUZ`, `NNYyh` and `kGBHZ` (see *Design*).

## What a person gets

An agent CLI connection (Claude Code or Codex) can **run on an API provider** instead of the CLI's own sign-in:

| Field | Rule |
|---|---|
| Interface | Implied by the CLI: Claude Code speaks the **Anthropic Messages** API, Codex the **OpenAI Responses** API. Choosing the interface chooses the CLI. |
| Base URL | Same rules as a native connection's (`BaseUrl`): `https://`, or `http://` for this computer only; no user name, password, query or fragment. E.g. DeepSeek: `https://api.deepseek.com/anthropic` (Claude Code), `https://api.deepseek.com` (Codex). |
| API key | A **project secret** (kind API key), picked or created with the connection, as native connections do. The connection stores the `SecretId` only. |
| Models | The model ids the provider serves, added by the person (one or more, kept once each, in order). They are the connection's model list: agent profiles on the connection choose from them. |

Agent profiles pick the connection and one of its models as for every connection; a run of such a profile starts the
CLI configured for the provider.

## Data model

- Core: `connection::ApiProvider { base_url: BaseUrl, key: SecretId, models }`
  (`crates/core/src/connection/api_provider.rs`), constructed by `ApiProvider::new`, which refuses an empty model list
  and drops repeated ids; its models carry no efforts. No new `CatalogSource` was added: that enum is matched
  exhaustively in the frozen `crates/desktop`, so a provider connection's `catalog()` is `None` and `models()` returns
  the provider's models instead.
- `Connection` gains `api_provider: Option<ApiProvider>`, set with `Connection::set_api_provider`. Only an agent CLI
  connection takes one (`ConnectionError::NoApiProvider` otherwise), and only while it runs on the CLI's Default
  account (`ConnectionError::ApiProviderWithAccount`); choosing another account on a provider connection is refused
  the same way. Plan limits, sign-in folders and the Accounts screen are about the CLI's own sign-in, which a provider
  connection does not use.
- `AgentProvider::provider_wire()` names the interface (`WireApi::AnthropicMessages` / `WireApi::OpenAiResponses`),
  `None` for native providers.
- On a provider connection:
  - `models()` are the provider's models; `catalog()` (the CLI's own listing) is `None`.
  - `status()` reads *Connected* once the program check found the CLI, whatever the CLI's own sign-in says (it is not
    used); `status_with(secrets)` reads *Key missing* while the key secret is archived or gone, as for native
    connections.
  - `vendor()` names the provider's host (`EngineVendor::Host`), since that host receives the prompts.
  - `plan_account()` is `None`: the runner records no plan-limit readings and keeps no account waits for it.
  - `key()` (on `Connection`) names the key secret of a native connection or a provider; the secrets' *Used by* and
    *needs a person to replace* rules use it.
- The Orchestrator chat and auto mode's reviewer start their CLI on its own sign-in; a connection on a provider is
  not offered there (`orchestrator.settings.apiProvider`, `reviewer_kind` = `None`) rather than run on the wrong
  credentials.
- Store: project database migration 72, `api_providers` (after master's `run_rebases`, 71), adds `connections.api_provider TEXT` (NULL, or
  `{"schema":1,"body":{base_url,key,models:[…]}}`). Older builds refuse the newer database as for every migration,
  so none of them runs the connection on the CLI's own sign-in by ignoring the provider.

## How each CLI is configured (verified against the CLIs' docs on 2026-10-18; no live provider call was made)

The key reaches **only the CLI process's environment**, as a `SecretVariable` (`Command::with_secret_env`), so the
runner's redaction set masks it in every event, error and transcript line.

**Claude Code** (`code.claude.com/docs/en/llm-gateway-connect`, `env-vars`):

| Variable | Value |
|---|---|
| `ANTHROPIC_BASE_URL` | the base URL |
| `ANTHROPIC_AUTH_TOKEN` | the key (secret), sent as `Authorization: Bearer`; it takes precedence over a saved login at once (`ANTHROPIC_API_KEY` would need an interactive approval) |
| `ANTHROPIC_MODEL`, `ANTHROPIC_DEFAULT_MODEL`, `ANTHROPIC_DEFAULT_OPUS_MODEL`, `ANTHROPIC_DEFAULT_SONNET_MODEL`, `ANTHROPIC_DEFAULT_HAIKU_MODEL`, `ANTHROPIC_DEFAULT_FABLE_MODEL`, `ANTHROPIC_SMALL_FAST_MODEL` (deprecated), `CLAUDE_CODE_SUBAGENT_MODEL` | the step's model, so aliases, background work (the Haiku slot) and subagents never ask the provider for a Claude model id it does not serve |
| `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB` | `1`: the processes Claude Code starts don't get the key |

Every other `ANTHROPIC_*` variable the project's environment sets is dropped (`ANTHROPIC_API_KEY` would reach the
provider's host in `x-api-key`, `ANTHROPIC_CUSTOM_HEADERS` as headers, the `ANTHROPIC_*_BASE_URL` variables would
route elsewhere), and so are the routing switches `CLAUDE_CODE_USE_{BEDROCK,VERTEX,FOUNDRY,MANTLE,ANTHROPIC_AWS}`,
`CLAUDE_CODE_SKIP_*_AUTH` of those, `CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST` and `CLAUDE_CODE_SUBAGENT_MODEL_FORCE`.
`--model` keeps naming the step's model. Claude Code already runs with `--setting-sources ""`, so no settings file
can override these.

**Codex** (`learn.chatgpt.com/docs/config-file/config-reference`, DeepSeek's Codex guide): App Server starts with

```
-c model_provider="orchester"
-c model_providers.orchester={name="Orchester API provider",base_url="<url>",env_key="ORCHESTER_PROVIDER_API_KEY",wire_api="responses"}
```

and the key in `ORCHESTER_PROVIDER_API_KEY` (secret). The runner adds the key to App Server's redaction set and,
since Codex needs no sign-in of its own there, no longer requires `account/read` to name an account. A model provider `orchester` in the user's own Codex configuration would merge with Orchester's
(its headers or query would reach requests carrying the key), so the step is refused then
(`refuse_taken_provider`), as for MCP server names. `wire_api = "responses"` is the only value current Codex takes,
so the provider must serve the OpenAI **Responses** API (DeepSeek does at `https://api.deepseek.com`); a server that
only speaks Chat Completions cannot be used through Codex, and the form says so. `thread/start` and `thread/resume`
name `modelProvider: "orchester"`, and the runner refuses the turn if Codex reports another `modelProvider` (Codex
0.159.2's schema makes it a required field of both answers).

## What is not supported, said where it matters

- **Plan limits**: none are read or shown for provider connections (they describe a CLI sign-in).
- **Reasoning effort** (the person's decision of 2026-10-18): Orchester does not know a provider's models, so a
  profile on a provider connection types its effort, optionally; empty uses the provider's default. Codex takes any
  one word its model advertises (App Server's `ReasoningEffort` is any non-empty string in 0.159.2) and gets it as the
  turn's `effort`; Claude Code's `--effort` takes only `low`, `medium`, `high`, `xhigh` and `max` (`claude --help`,
  2.1.301), so for Claude Code the value is checked against that set with a clear error (`provider_effort`).
- **Tokens**: counted as before whenever the CLI reports them — Claude Code's stream-json `usage`, Codex's
  `thread/tokenUsage/updated` — which depends on the provider returning usage in its API's format (DeepSeek does).
  Nothing estimates tokens a CLI does not report.
- **Model listing from the endpoint**: not offered; the existing `list_models` path only reads a connection's stored
  catalog, so the models are typed. The runner's native *Test connection* (`GET /models`) could serve it later.
- **Images, tools, thinking**: whatever the provider accepts of what the CLI sends; Orchester does not translate. A
  provider that rejects a request shows as the CLI's error in the run (e.g. DeepSeek's Anthropic route ignores
  `cache_control` and MCP tool blocks).
- **Who can read the key** (probed 2026-10-18 on Windows with fake values and no model turn; scratch scripts in
  `.tmp/api-providers/`):
  - Claude Code 2.1.301 with `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=1`: a stdio MCP server started from the MCP
    configuration no longer sees `ANTHROPIC_AUTH_TOKEN` (without the scrub it does), while the variables of its own
    configuration stay (`PROBE_TOKEN` stayed), so Orchester's permission-prompt relay keeps its token. That Bash
    commands and hooks are scrubbed too is the docs' word (`code.claude.com/docs/en/env-vars`, *What the subprocess
    environment scrub removes*); proving it needs a model turn. The scrub also removes other credential-looking
    variables (by name or value) from the commands of a provider run, and `CLAUDE_CONFIG_DIR`.
  - Codex 0.159.2: `-c shell_environment_policy.ignore_default_excludes=false` keeps `ORCHESTER_PROVIDER_API_KEY` out
    of commands (`codex sandbox` with a fake key: gone; a user's own `exclude` list still applied). An `exclude=[…]`
    override was rejected: it replaced the user's own list (probed). Side effect: on a provider run, project variables
    whose names contain KEY, SECRET or TOKEN don't reach the commands either. Codex starts stdio MCP servers with a
    short whitelist only: a probe server saw neither the key nor any other variable of Codex's environment.
  - The CLI process itself holds the key, as it must to call the provider.
- **Telemetry**: Claude Code still sends its nonessential traffic (telemetry, update checks) to Anthropic without the
  provider's key; `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` would stop it (and auto-updates). Left off: the
  person's choice.
- **Removing a connection**: Orchester has no removal of agent connections; *Edit connection* can switch a connection
  back to the CLI's own sign-in, which removes its provider.
- **Plan limits**: a step stopped at an account's plan limit is never offered to continue on a provider connection
  (`AgentConnection::continues_plan_limited`), in the window or the runner.
- **Sessions**: a session continues only on what it was made on (`SessionOrigin`): the account (as before) and the
  provider's base URL and key secret, recorded per invocation with the invocation's engine (`InvocationEngine::key`).
  Another host or key, a provider where there was none, or none where there was one, starts a new session. A resumed
  Codex thread is checked for its `modelProvider` like a new one.
- **Orchester's tools' answers** leave out the provider key as well as the MCP servers' secrets.

## Domain MCP tools and the person gate

- `add_cli_connection` takes an optional `api_provider: {base_url, key_secret, models}`; `key_secret` names an
  **existing** project secret by id or name (no tool takes or returns a secret value).
- `set_api_provider {project, connection, api_provider | null}` changes or removes it.
- Adding a provider, or changing its base URL or key, sends the project's prompts and code to another host or under
  another key: refused `person_only` unless a person turned on *Let automation change agent permissions*
  (`widening::allowed`, reason `appMcp.widens.apiProvider`, together with any other widening of the same call, such as
  a program not found on the search path). Changing only the models, or removing the provider (back to the CLI's
  sign-in), is not a widening.
- In the window, saving a connection on a provider with lurq's synthetic input (an MCP client driving the window) is
  taken only where `input_may_decide(.., Guarded::PermissionWidening)` allows it, as for choosing an account.
- `list_connections` / `list_models` show the provider's host, base URL, key secret's name and models, never a value.

## Design

- *Add connection* (`virfc`, states `UW2bO`) → Codex CLI / Claude Code CLI: the check row reads *Found · <CLI>
  <version> · runs on the API provider; its own sign-in is not used* on a provider; a *Runs on* section after the
  connection section (`screens/connections/provider_form/`) — *Its own sign-in* (the Account row stays in the
  connection section) or *An API provider*, which hides the Account row and shows Base URL (with the CLI's interface
  as its description), API key (a select of the project's secrets, the archived one a connection still names as
  *<name> · archived* with its error, or *New secret…* with its name and masked key under it, kept in Development),
  Models (removable chips with a field and *Add model*, as allowed commands are added: a decision of 2026-10-18 over
  the frame's comma field), where requests go and what is not supported, then the next step.
- *Edit connection* (`VvWpe`, `FsIt3`): the properties, the change note, the Account row on the sign-in, the same
  section filled from the stored provider, and when the page takes a connection off its provider *Saving takes
  <name> off the API provider*; saving goes through `set_runs_on_in`.
- The inspector (`y2qhJ`, `tnEKp`, `b3aoUZ`): *Authentication · API key*, *Plan limits · Not read on an API
  provider*, *Runs on* (Interface, Base URL, API key by name or *Key secret missing*, where requests go), the typed
  models (no Refresh), and the footer sentence for Connected or Key missing (Edit first then). The list row reads
  *<CLI> <version> · <host>*.
- Agent profiles (`NNYyh`, `kGBHZ`): the connection menu names the host; the model menu lists the provider's models;
  the effort is an optional text field (*Provider default* when empty). The Orchestrator's and auto mode's reviewer's
  menus list provider connections, unavailable, with why.
- QA: the seed scenario `api-providers` (Claude · DeepSeek, Codex · DeepSeek, Claude · Gateway not checked, Claude ·
  Old gateway with an archived key, profile Researcher on Codex · DeepSeek).

## Decisions taken here, with the alternative

1. **Per project, key in a project secret** (as native connections). Alternative: app-wide providers like accounts
   (Settings › Accounts, key in the OS secret store under an app key), chosen by connections in every project.
2. **On the connection** (program + provider = connection, like program + account). Alternative: on the agent
   profile.
3. **Bearer token for Claude Code** (`ANTHROPIC_AUTH_TOKEN`). Alternative: a per-connection choice of `x-api-key`
   (`ANTHROPIC_API_KEY`) for gateways that read only that header.
4. **Models typed** in v1. Alternative: fetch with the runner's `GET /models` check.

