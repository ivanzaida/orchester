# API providers: agent CLIs on third-party model APIs

Asked 2026-10-18: *the ability to add agents from DeepSeek etc., through an API key, with an Anthropic/Codex
interface.* This note is the design of what was built on branch `claude/api-providers`; it supersedes decision 7 of
`docs/architecture/native-agent.md` (*no custom base URL or key for CLI connections*), which the person's request
overrides. The native agent (Orchester's own loop on a model API) is unchanged and separate.

No Pencil frame exists for any of this. The UI below is built from the closest existing connection-form patterns and
needs a design (see *Design needed*).

## What a person gets

An agent CLI connection (Claude Code or Codex) can **run on an API provider** instead of the CLI's own sign-in:

| Field | Rule |
|---|---|
| Interface | Implied by the CLI: Claude Code speaks the **Anthropic Messages** API, Codex the **OpenAI Responses** API. Choosing the interface chooses the CLI. |
| Base URL | Same rules as a native connection's (`BaseUrl`): `https://`, or `http://` for this computer only; no user name, password, query or fragment. E.g. DeepSeek: `https://api.deepseek.com/anthropic` (Claude Code), `https://api.deepseek.com` (Codex). |
| API key | A **project secret** (kind API key), picked or created with the connection, as native connections do. The connection stores the `SecretId` only. |
| Models | The model ids the provider serves, typed by the person (one or more, kept once each, in order). They are the connection's model list: agent profiles on the connection choose from them. |

Agent profiles pick the connection and one of its models as for every connection; a run of such a profile starts the
CLI configured for the provider.

## Data model

- Core: `connection::ApiProvider { base_url: BaseUrl, key: SecretId, models: ModelCatalog }`
  (`crates/core/src/connection/api_provider.rs`), constructed by `ApiProvider::new`, which refuses an empty model list
  and drops repeated ids. The catalog's source is `CatalogSource::ApiProvider` (typed by a person), its models carry
  no efforts.
- `Connection` gains `api_provider: Option<ApiProvider>`, set with `Connection::set_api_provider`. Only an agent CLI
  connection takes one (`ConnectionError::NoApiProvider` otherwise), and only while it runs on the CLI's Default
  account (`ConnectionError::ApiProviderWithAccount`); choosing another account on a provider connection is refused
  the same way. Plan limits, sign-in folders and the Accounts screen are about the CLI's own sign-in, which a provider
  connection does not use.
- `AgentProvider::provider_wire()` names the interface (`WireApi::AnthropicMessages` / `WireApi::OpenAiResponses`),
  `None` for native providers.
- On a provider connection:
  - `models()` / `catalog()` are the provider's models, not the CLI's own catalog.
  - `status()` reads *Connected* once the program check found the CLI, whatever the CLI's own sign-in says (it is not
    used); `status_with(secrets)` reads *Key missing* while the key secret is archived or gone, as for native
    connections.
  - `vendor()` names the provider's host (`EngineVendor::Host`), since that host receives the prompts.
  - `plan_account()` is `None`: the runner records no plan-limit readings and keeps no account waits for it.
- Store: project database migration `api_providers` adds `connections.api_provider TEXT` (NULL, or
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
| `ANTHROPIC_MODEL`, `ANTHROPIC_DEFAULT_OPUS_MODEL`, `ANTHROPIC_DEFAULT_SONNET_MODEL`, `ANTHROPIC_DEFAULT_HAIKU_MODEL`, `CLAUDE_CODE_SUBAGENT_MODEL` | the step's model, so aliases, background work (the Haiku slot) and subagents never ask the provider for a Claude model id it does not serve (`ANTHROPIC_SMALL_FAST_MODEL` is deprecated) |

`--model` keeps naming the step's model. Claude Code already runs with `--setting-sources ""`, so no settings file
can override these.

**Codex** (`learn.chatgpt.com/docs/config-file/config-reference`, DeepSeek's Codex guide): App Server starts with

```
-c model_provider="orchester"
-c model_providers.orchester={name="Orchester API provider",base_url="<url>",env_key="ORCHESTER_PROVIDER_API_KEY",wire_api="responses"}
```

and the key in `ORCHESTER_PROVIDER_API_KEY` (secret). `wire_api = "responses"` is the only value current Codex takes,
so the provider must serve the OpenAI **Responses** API (DeepSeek does at `https://api.deepseek.com`); a server that
only speaks Chat Completions cannot be used through Codex, and the form says so. `thread/start` and `thread/resume`
name `modelProvider: "orchester"`, and the runner refuses the turn if Codex reports another `modelProvider` (Codex
0.159.2's schema makes it a required field of both answers).

## What is not supported, said where it matters

- **Plan limits**: none are read or shown for provider connections (they describe a CLI sign-in).
- **Reasoning effort**: typed models offer no effort, so profiles on them set none; the provider's default applies.
- **Tokens**: counted as before whenever the CLI reports them — Claude Code's stream-json `usage`, Codex's
  `thread/tokenUsage/updated` — which depends on the provider returning usage in its API's format (DeepSeek does).
  Nothing estimates tokens a CLI does not report.
- **Model listing from the endpoint**: not offered; the existing `list_models` path only reads a connection's stored
  catalog, so the models are typed. The runner's native *Test connection* (`GET /models`) could serve it later.
- **Images, tools, thinking**: whatever the provider accepts of what the CLI sends; Orchester does not translate. A
  provider that rejects a request shows as the CLI's error in the run (e.g. DeepSeek's Anthropic route ignores
  `cache_control` and MCP tool blocks).
- **The key and the agent's commands**: Claude Code and Codex pass their environment to the commands the agent runs,
  so those commands can read the key, as they would if the person started the CLI with the variable themselves.
  Claude Code's `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB` and Codex's `shell_environment_policy.ignore_default_excludes=false`
  could strip it; neither is turned on because the first also strips stdio MCP servers' variables (Orchester's own
  permission prompt relay among them, unverified) and the second also drops project variables whose names contain
  KEY, SECRET or TOKEN. A decision for the person.
- **Removing a connection**: Orchester has no removal of agent connections; *Edit connection* can switch a connection
  back to the CLI's own sign-in, which removes its provider.

## Domain MCP tools and the person gate

- `add_cli_connection` takes an optional `api_provider: {base_url, key_secret, models}`; `key_secret` names an
  **existing** project secret by id or name (no tool takes or returns a secret value).
- `set_api_provider {project, connection, api_provider | null}` changes or removes it.
- Adding a provider, or changing its base URL, key or models, sends the project's prompts and code to another host
  under another key: refused `person_only` unless a person turned on *Let automation change agent permissions*
  (`widening::allowed`, reason `appMcp.widens.apiProvider`). Removing the provider (back to the CLI's sign-in) is a
  narrowing and always allowed.
- `list_connections` / `list_models` show the provider's host, base URL, key secret's name and models, never a value.

## UI placement (desktop-app)

- *Add connection* → Codex CLI / Claude Code CLI: a new *Runs on* choice after the program — *Its own sign-in*
  (the existing Account row) or *An API provider*: Base URL, API key (existing secret select, or *New secret…* with
  name, environment and value, as the Secrets page), Models (comma-separated ids), and the interface note.
- *Edit connection* shows the same section for an agent CLI connection.
- The inspector of a provider connection shows Interface, Base URL, API key (secret name), Models and *Prompts,
  repository content and command output are sent to <host>*; it shows no account or plan rows.
- Agent profiles: the model menu lists the provider's models; the connection detail names the host.

## Decisions taken here, with the alternative

1. **Per project, key in a project secret** (as native connections). Alternative: app-wide providers like accounts
   (Settings › Accounts, key in the OS secret store under an app key), chosen by connections in every project.
2. **On the connection** (program + provider = connection, like program + account). Alternative: on the agent
   profile.
3. **Bearer token for Claude Code** (`ANTHROPIC_AUTH_TOKEN`). Alternative: a per-connection choice of `x-api-key`
   (`ANTHROPIC_API_KEY`) for gateways that read only that header.
4. **Models typed** in v1. Alternative: fetch with the runner's `GET /models` check.

## Design needed

Pencil frames for: the *Runs on* section of Add/Edit connection (both CLIs, existing and new secret, errors), the
provider connection's inspector and list row, and the profile's connection detail with a provider.
