# Connecting agents to the app's MCP server: a stable address in Settings

Proposed 2026-10-13. The person chose the direction (a stable address and token set in Settings); the open decisions
are at the end. The design source is `data/orchester2.pen`, Settings › Automation (frames listed under
[Frames](#frames)). Read the encrypted document only through Pencil. Ports, client names, versions, times, the
masked token and its fake characters in the frames are illustrative fixture data. They are not defaults to ship,
except the proposed default port `47821`, which is open decision 1.

## The problem

`orchester-app` runs one MCP server of its own, with lurq's window tools, the app's tools (`app_state`, `navigate`,
`set_theme`, `hold_reads`) and the domain tools (`crates/desktop-app/src/mcp/`). Today (checked in the code
2026-10-13):

- `server.rs` binds `127.0.0.1:0`, so the OS picks a free port on every start. `AppMcp::start` (`mod.rs`) makes a new
  `BearerToken::generate()` on every start too.
- `discovery/` writes both into `%LOCALAPPDATA%\orchester-app-mcp\<pid>.json` (macOS:
  `~/Library/Application Support/orchester-app-mcp/<pid>.json`). Only the user can read the file (icacls / `0600`),
  and a `<pid>.lock` cleans up after killed processes.
- `McpStatus` carries the port only. The status bar shows `MCP on :{port}` / `MCP unavailable`
  (`shell.status.mcpListening`). The window never shows the token.
- Clients find the file by pid (`tools/lurq-mcp-client.py <pid> …`, the runs' `call_run_app` relay,
  `tools/qa-app.ps1`).

This breaks in daily use:

1. The pid, port and token change on every restart, so a registered client is stale after each restart.
2. A Claude desktop session on Windows reads a stale, virtualized copy of `%LOCALAPPDATA%` (most likely MSIX
   AppData virtualization) and never finds the current file.
3. Codex's sandbox cannot read the private discovery file. That is correct: agents inside runs must not control the
   app. But it also means a person's own Codex session cannot find the server.

## The model

**The installed app listens on one address the person sets in Settings and accepts one token kept in the OS secret
store. Each agent CLI is registered once with a command Settings gives, and the agent then sees the app's tools
directly as `mcp__orchester__*`. There is no pid, no discovery file and no Python client.**

- **Address.** `http://127.0.0.1:<port>/mcp`, loopback only (as today), default port `47821`. The person changes
  the port in Settings › Automation; *Use port* restarts the server on the new port in place, with no app restart.
  If the port is taken, the server does not listen, and Settings and the status bar say so. It never falls back to
  another port: a client registered with the stable address would silently reach nothing, or something else.
- **Token.** Generated once (the current `BearerToken::generate`, 244 random bits) when the stable address is first
  used, stored in the OS secret store, never in a file, the database, the repository, logs or the MCP tree. *Show* and
  *Copy* work only on a person's click in the window. *Rotate token* replaces it after a confirmation in place, and
  every registered client must be registered again.
- **Server on/off.** A switch, *Let agents connect to Orchester*. Off means nothing listens: the status bar shows
  *MCP off*, and the token stays stored for when it is turned on again.
- **Connect an agent.** One block with a tab per CLI (Claude Code, Codex). Each shows the exact command or
  configuration for the current address with the token masked, plus *Copy*, which puts the full command with the
  token on the clipboard.
- **Who is connected.** The server knows its open MCP sessions (rmcp's `LocalSessionManager`) and the `clientInfo`
  each sent with `initialize`. Settings lists them as *the client says it is …*, with *connected now* or *last
  request N min ago*. This list is kept in memory since the app started; nothing is stored and nothing is inferred
  beyond what the client sent.
- **What automation may do.** The existing person-only rows keep their place and behaviour under their own section
  title, *What automation may do*: window control, *Let automation answer approvals*, *Let automation change agent
  permissions*. The MCP section only states the relation in its description: *What connected agents may do is set
  below, in What automation may do.* None of the new rows can be changed through the MCP server.
- **QA copies and tests** keep today's mechanism: a free port, a token per start (not stored in the secret store),
  and a discovery file. The file goes into a folder that an environment variable names
  (`ORCHESTER_APP_MCP_DISCOVERY=<folder>`); `tools/qa-app.ps1` sets it to `.tmp/qa/<task>/mcp/` in the worktree.
  While the variable is set, the stable address and the stored token are not used. The MCP section is replaced by
  one line, *This copy was started for QA: a new port each start, its discovery file is in {folder}*, and the
  status-bar hint names that folder. No other UI.

### Modes and builds

| Started as | Address | Token | Discovery file |
|---|---|---|---|
| Installed release build (`tools/install.ps1`, `Orchester App.app`) | Stable, from Settings (default `47821`) | Stored in the OS secret store | None |
| `ORCHESTER_APP_MCP_DISCOVERY` set (QA copies, test harnesses) | Free port per start | New per start, in memory only | `<folder>/<pid>.json`, as today |
| Debug build (`tools/run.ps1 app`) without the variable | See open decision 2 (recommended: free port per start, discovery file in the user folder as today) | New per start | User folder, as today |

Each build keeps its own `orchester.db` beside the executable (`orchester_store::location`). The stable-mode
settings (port, on/off) are therefore per installation, and a dev build never takes the installed app's port by
accident.

## Frames

Root layer **Settings** (`uyO5v`) in `data/orchester2.pen`, at the bottom of its column, below
`Settings · Servers · empty and could not be read`. They replace the *Orchester MCP server* section of
`Settings · Automation · dark` (`oD7hy`), which shows today's per-start port and access file; `oD7hy`, its states
board `HNQBR` and the popover board `Cpav5` are left as they are and become outdated once this is approved. Every
frame is dark and has its `Label · …` 32 px above. Only existing components are used (`Form row` `XaNIz`,
`Form row/stacked` `WXgjE`, `Form section title` `De1nE`, `Command row` `PEnj5`, `Masked value` `DShSq`,
`In-place confirmation` `RWODj`, `Segmented/text` `S5Fsve`, `Field/select` `pV2IA`, `Field/text` `JFXZg`,
`Field error` `d5xipm`, `Inspector note` `vCguQ`, `Notice` `R3Pyx`, buttons, switches, `Status item` `wnnsD`); no new
component. The tab strip follows the Accounts design: General · Automation · Accounts 5 · Servers 4. Exports at
scale 1: `.tmp/design-mcp-settings/<node id>.png`.

| Node | Frame | What it shows |
|---|---|---|
| `lye6f` | Settings · Automation · MCP server · stable address · dark (1440 × 1320) | Section *Orchester MCP server* (meta *This installation*). *Let agents connect to Orchester*: on, *Listening*. *Address*: `http://127.0.0.1:47821/mcp` with *Copy*, a port field and *Use port*. *Access token*: masked, with *Show* / *Copy* / *Rotate token*. *Connect an agent*: Claude Code selected, *Available: In every folder (user)*, the command with the masked token, *Copy*, a clipboard note. *Agents connected*: two clients, as the clients name themselves. *What automation may do* is unchanged. The status bar shows `MCP 127.0.0.1:47821` |
| `NzOSY` | Settings · Automation · Connect an agent · Codex, scopes and macOS · dark (board) | Codex (Windows) in two steps: `codex mcp add … --bearer-token-env-var ORCHESTER_APP_MCP_TOKEN`, then `setx`, with the line that other programs running as you can read the variable; the `config.toml` entry below as the alternative. Claude Code *In one folder only (local)*. macOS: token in the login keychain, Codex with `launchctl setenv` + `~/.zshrc`. *Copied* state. Updated 2026-10-13 to the decided Codex default (cells `E3CaLR`, `DOYRs`) |
| `eM8tB` | Settings · Automation · Access token · rotate in place and show · dark (board) | *Rotate the token?* in place under the row (*Rotate token* danger / *Keep current token*). Rotated: success line (re-register; for Codex, set the variable again), *No agent connected since the token changed*. Token shown (fake value, *Hide*, *Hidden again in 24 s*, screenshots refused). Store failed with *Try again*. Input from automation not taken |
| `eTrCO` | Settings · Automation · MCP server · states · dark (board, each cell with its status bar) | Port taken (port field focused, *Use port* primary, error line, `MCP port 47821 taken`). Port changed (`MCP 127.0.0.1:47822`, re-register line). Server off (`MCP off`). Token unreadable (danger notice, *Try again*, `MCP unavailable`). QA copy (`MCP :53122 · QA`, discovery line). Development build (open decision 2) |
| `QqBCj` | Status bar · MCP item · stable address · states · dark (board) | The popover of the MCP item (the pattern of `Cpav5`): listening, with the address, *Copy*, *2 agents connected*, the approvals switch and the window-control line; port taken; off; QA copy with its discovery file and window control on for the run |

Label nodes: `WohF9`, `m0yj1e`, `hKH7z`, `V2dMM`, `jMWgL`. Changed outside the new frames: only the height of the
Settings layer `uyO5v` (16728 → 23676), so the new frames sit inside it.

## States and copy

Proposed keys in `crates/desktop-app/assets/locales/en/settings.json` (namespace `settings.automation.server.*` and
`settings.automation.connect.*`) and `shell.json` (`shell.status.mcp*`). Placeholders as in the existing locale
files (`{{name}}`).

### Section *Orchester MCP server*

| Key | English |
|---|---|
| `settings.automation.server.section` | Orchester MCP server |
| `settings.automation.server.description` | Agents you connect here can read and change your projects through Orchester, with the rules of its screens. What they may do beyond that is set below, in What automation may do. Only programs on this computer can connect. |
| `settings.automation.server.enabled.title` | Let agents connect to Orchester |
| `settings.automation.server.enabled.off` | Off: nothing listens. Agents you registered can't reach Orchester until you turn this on. |
| `settings.automation.server.address.title` | Address |
| `settings.automation.server.state.listening` | Listening |
| `settings.automation.server.state.portTaken` | Port {{port}} taken |
| `settings.automation.server.state.off` | Off |
| `settings.automation.server.state.notRunning` | Not running |
| `settings.automation.server.meta` | This installation / This QA copy / This development build (`meta.installation`, `meta.qa`, `meta.development`) |
| `settings.automation.server.address.description` | Fixed, so an agent you register once keeps connecting after Orchester restarts. If the port is taken, Orchester says so here and doesn't move to another one. |
| `settings.automation.server.address.copy` | Copy |
| `settings.automation.server.port.label` | Port |
| `settings.automation.server.port.use` | Use port |
| `settings.automation.server.port.invalid` | Use a port from 1024 to 65535. |
| `settings.automation.server.port.taken` | Port {{port}} is in use by another program, so Orchester isn't listening. It doesn't switch to another port on its own: agents registered with this address would reach nothing. Choose another port, then register your agents again. |
| `settings.automation.server.port.takenBy` | Port {{port}} is in use by {{program}} (process {{pid}}). (Only if open decision 5 is accepted.) |
| `settings.automation.server.port.changed` | Now listening on port {{port}}. Agents registered with port {{old}} need registering again. |
| `settings.automation.server.token.title` | Token |
| `settings.automation.server.token.where` | Kept in Windows Credential Manager. Agents send it with every request. / Kept in your login keychain. Agents send it with every request. (`whereWindows`, `whereMacos`) |
| `settings.automation.server.token.show` | Show |
| `settings.automation.server.token.hide` | Hide |
| `settings.automation.server.token.shownFor` | Hidden again in {{seconds}} s. Screenshots of the window are refused while it shows. |
| `settings.automation.server.token.copy` | Copy |
| `settings.automation.server.token.copied` | Copied. Orchester clears it from the clipboard in 1 minute. |
| `settings.automation.server.token.rotate` | Rotate token |
| `settings.automation.server.token.rotateConfirm.title` | Rotate the token? |
| `settings.automation.server.token.rotateConfirm.body` | Every agent registered with the current token stops connecting until you register it again with the new one. Open sessions end now. |
| `settings.automation.server.token.rotateConfirm.confirm` | Rotate token |
| `settings.automation.server.token.rotateConfirm.cancel` | Keep current token |
| `settings.automation.server.token.rotated` | New token in use. Register your agents again with the commands below; for Codex, set ORCHESTER_APP_MCP_TOKEN again. |
| `settings.automation.server.token.unreadable` | The token can't be read from {{store}}: {{reason}}. The server is off until it can. |
| `settings.automation.server.token.tryAgain` | Try again |
| `settings.automation.server.blocked` | A change brought by automation was not taken: only you change the MCP server's settings, here in the window. |
| `settings.automation.server.qa` | This copy was started for QA: a new port on each start; its discovery file is in {{folder}}. The address and token settings apply to normal starts. |

### *Connect an agent*

| Key | English |
|---|---|
| `settings.automation.connect.title` | Connect an agent |
| `settings.automation.connect.description` | Register Orchester once. The agent then has Orchester's tools in every session, as mcp__orchester__… |
| `settings.automation.connect.tab.claude` | Claude Code |
| `settings.automation.connect.tab.codex` | Codex |
| `settings.automation.connect.scope.label` | Available |
| `settings.automation.connect.scope.user` | In every folder (user) |
| `settings.automation.connect.scope.local` | In one folder only (local) |
| `settings.automation.connect.claude.hint` | Run this in a terminal. With "In one folder only", run it in that folder. |
| `settings.automation.connect.codex.step1` | Register Orchester in Codex. Codex reads the token from a variable, not from its configuration: |
| `settings.automation.connect.codex.step2Windows` | Set the variable for your user. It reaches programs started after this, so restart Codex, your terminal or your IDE: |
| `settings.automation.connect.codex.step2Macos` | Set the variable for your user. launchctl setenv reaches apps started from the Dock or Finder until you sign out; add the same export to ~/.zshrc for terminals: |
| `settings.automation.connect.codex.readersWindows` | setx keeps the variable in your Windows user profile, and every program you start from then on gets it, so other programs running as you can read the token. Agents in Orchester's runs never get it: their environment is cleaned. After Rotate token, set it again. Check with codex mcp get orchester. |
| `settings.automation.connect.codex.readersMacos` | The variable is in the environment of every program you start from then on, so other programs running as you can read the token. Agents in Orchester's runs never get it: their environment is cleaned. After Rotate token, set it again. Check with codex mcp get orchester. |
| `settings.automation.connect.codex.alternative` | Or put the token in ~/.codex/config.toml instead (no variable; any program running as you can read that file): |
| `settings.automation.connect.copy` | Copy command |
| `settings.automation.connect.copied` | Copied with the token. Orchester clears it from the clipboard in 1 minute. |
| `settings.automation.connect.check` | To check: claude mcp get orchester / codex mcp get orchester |
| `settings.automation.connect.never` | Never put the token in a repository's .mcp.json or AGENTS.md. |
| `settings.automation.connected.title` | Agents connected |
| `settings.automation.connected.none` | No agent connected since Orchester started. |
| `settings.automation.connected.client` | {{name}} {{version}} |
| `settings.automation.connected.now` | connected now |
| `settings.automation.connected.lastSeen` | last request {{time}} |
| `settings.automation.connected.says` | Names and versions are what each client says it is. |

### Status bar

| Key | English |
|---|---|
| `shell.status.mcpListening` (changed) | MCP {{address}} (e.g. `127.0.0.1:47821`) |
| `shell.status.mcpPortTaken` | MCP port {{port}} taken |
| `shell.status.mcpOff` | MCP off |
| `shell.status.mcpUnavailable` | MCP unavailable (kept; now also for an unreadable token) |
| `shell.status.mcpQa` | MCP :{{port}} · QA |
| `shell.status.mcpHint.address` | Orchester's MCP server · {{url}} |
| `shell.status.mcpHint.token` | Token in {{store}} · Settings › Automation |
| `shell.status.mcpHint.clients` | {{count}} agents connected (plural `one`/`other`) |
| `shell.status.mcpHint.qa` | QA copy · discovery file in {{folder}} |

Clicking the MCP item opens its popover, as in `Cpav5`, drawn for the new states in `QqBCj`. The popover keeps
*Let automation answer approvals* and *Open Automation settings*. It names the address and where the token is kept,
never the token, and says how many agents are connected (`shell.status.mcpHint.*` above are its lines). The
*Agents can control this window* item (`shell.status.windowControl`) stays as it is.

## Client commands

Checked 2026-10-13 on Windows 11 with **Claude Code 2.1.296** and **Codex CLI 0.159.2**: `--help` of each, plus
registration into empty scratch homes under `.tmp/design-mcp-settings/probe/` (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`)
with a fake token. Nothing connected to a server.

### Claude Code

```
claude mcp add --transport http --scope user orchester http://127.0.0.1:47821/mcp --header "Authorization: Bearer <token>"
```

- Verified: `-t/--transport http`, `-H/--header`, `-s/--scope local|user|project` (default `local`). The command
  wrote `mcpServers.orchester = {type: "http", url, headers: {Authorization: "Bearer …"}}` into `.claude.json` of the
  config home, and its own output redacted the header (`"Authorization": "[REDACTED]"`).
- `user` makes it available in every folder; `local` (the default) only in the folder it is run in. Both live in
  `~/.claude.json`, outside any repository. **`project` is not offered**: it writes `.mcp.json` into the repository,
  which would commit the token.
- On PowerShell the command works as shown (double quotes). On macOS zsh, the same.
- Verified: `claude mcp add-json` accepts `{"type":"http","url":…,"headersHelper":"<command>"}`, a command whose
  output supplies the headers at connect time. **Unverified**: the helper's output format and when it runs. See
  open decision 4.
- Verified 2026-10-14 against a QA copy: Claude Code 2.1.296 opens with a `server/discover` probe for MCP 2026-07-28.
  The server speaks the revisions up to 2025-11-25 only and declines it (HTTP 400, JSON-RPC `-32022` listing them);
  Claude Code then initializes a session on 2025-11-25 and lists the tools. Advertising 2026-07-28 made it take that
  revision's stateless path and reject `tools/list` for lacking `ttlMs` and `cacheScope`.
- **Unverified**: that the Claude desktop app's Code sessions on Windows read the same `~/.claude.json` and can reach
  `127.0.0.1` (a full-trust MSIX package can use loopback; AppData virtualization does not affect `~/.claude.json`).
  This must be checked once, with the desktop app, after implementation.

### Codex

Default (decided 2026-10-13): register with a command, and keep the token in a user environment variable.

```
codex mcp add orchester --url http://127.0.0.1:47821/mcp --bearer-token-env-var ORCHESTER_APP_MCP_TOKEN
```

then set the variable for the user:

- Windows: `setx ORCHESTER_APP_MCP_TOKEN <token>`. It is kept in the user's profile (`HKCU\Environment`) and reaches
  programs started after it, so Codex, the terminal or the IDE must be restarted.
- macOS: `launchctl setenv ORCHESTER_APP_MCP_TOKEN <token>` for apps started from the Dock or Finder (until the
  person signs out), plus `export ORCHESTER_APP_MCP_TOKEN=<token>` in `~/.zshrc` for terminals.

Notes:

- Verified: `codex mcp add` writes `url` and `bearer_token_env_var = "ORCHESTER_APP_MCP_TOKEN"` and prints
  *Added global MCP server 'orchester'*. `codex mcp add` has **no flag for a literal token or header** (`--help`).
- **Who can read the variable:** every program the person starts from then on gets it in its environment, so other
  programs running as the same user can read the token. (On Windows they can also read it from the registry, and
  any such program could read Credential Manager or the client config files too.) Other users can't. The Settings
  copy says this under the variable step.
- Agents in Orchester's runs don't inherit it. The name is chosen apart from `ORCHESTER_MCP_TOKEN_*`, the prefix the
  runner uses for MCP servers' bearer tokens in runs, and it is not on the agent environment's allow-list
  (`connection/invocation/mod.rs`, `ALLOWED`). That needs a test when implemented.
- After *Rotate token* the variable must be set again; the rotated line says so for Codex.
- **Unverified:** that Codex reads the variable when Codex runs as an IDE extension or the Codex app. The extension
  or app must have been started after `setx` / `launchctl setenv`.

Alternative (shown below, with its own *Copy*): an entry in `~/.codex/config.toml`, with no variable.

```toml
[mcp_servers.orchester]
url = "http://127.0.0.1:47821/mcp"
http_headers = { Authorization = "Bearer <token>" }
```

- Verified: Codex 0.159.2 loads it. `codex mcp list --json` shows `transport.type = "streamable_http"`, the header
  and `auth_status: "bearer_token"`.
- The token then sits in that file, which any program running as the person can read. It is the alternative for
  people who don't want the token in every program's environment.
- Verified, not used: the config also has `env_http_headers` and `http_headers_helper` (a string; its semantics are
  **unverified**; header helpers come later).

### The server name

The registered name is **`orchester`**, never `Orchester`. Runs give Codex agents the runner's relay as
`mcp_servers.Orchester` through `-c` overrides, and a user table of the same name would merge into it
(`app_server_mcp.rs`: tables merge with the user's). Settings fixes the name in the commands; it is not a field.

## Security

- **Loopback only.** The server binds `127.0.0.1` (as today). Other machines can't connect.
- **Token storage.** It goes in the OS secret store through `orchester-vault` (`KeychainVault`), with a new
  `VaultKey::app_mcp_token(installation)`: service `orchester-app-mcp`, account `<installation id>`. Windows target
  `orchester-app-mcp/<installation id>`, `Local` persistence; macOS generic password in the login keychain. The
  installation id is a random id stored in that installation's `orchester.db`. The dev build and the installed app
  then never share a token, and removing an installation's database orphans only its own credential (*Rotate* and
  the uninstall step remove it).
- **Who can read the token.** Any program running as the person can read a Windows Credential Manager generic
  credential, the configuration files of the registered clients (`~/.claude.json`, and `~/.codex/config.toml` with the
  alternative), and the user environment variable `ORCHESTER_APP_MCP_TOKEN` of the Codex default (inherited by every
  program the person starts, and on Windows also stored in `HKCU\Environment`). This
  is the same exposure as today's discovery file: the server adds no access a program running as the person lacks
  (`docs/stable-build.md`, "What this means"). Other users of the computer can connect to the port but can't read
  the token. On macOS the keychain item's access list admits the app. Ad-hoc signing (today's `.app`) may make
  macOS ask again after each update; the vault's project secrets have the same issue, and it is unverified on a Mac.
- **Never shown by automation.**
  - *Show*, *Copy*, *Rotate token*, *Use port*, the server switch and the scope select are person-only. They join
    `mcp::input_gate` (a new `Guarded::McpServer`), so synthetic input is not taken and the row says so
    (`…server.blocked`).
  - While the token is shown in clear (30 s at most), the server refuses `lurq_screenshot` and the element tree's
    text with a refusal that names the reason. In the tree the token is always the masked text.
  - `app_state` and every tool result name the address and the token's *presence*, never its value.
  - `BearerToken` keeps its redacting `Debug` and gets no `Display`. Logs name the address, never the token.
- **Clipboard.** *Copy* (token or command) puts the token on the clipboard, which any program running as the person
  can read. Orchester clears it after 60 s if the clipboard still holds what it put there.
- **Rotation.** A new token is stored first, the server switches to it, and the old sessions are cancelled
  (`StreamableHttpServerConfig::cancellation_token`, as on stop). A failed store keeps the old token and says so in
  place.
- **Agents in runs** can't reach the server through their tools:
  - Claude Code agents start with `--strict-mcp-config --setting-sources ""` (`invocation/claude.rs`), so the user's
    `orchester` server is not loaded.
  - Codex agents get the user's servers turned off by name, and the thread's inventory is verified
    (`app_server_mcp.rs`).
  - Neither inherits `ORCHESTER_APP_MCP_TOKEN` (cleaned environment).
  - An agent's *commands* could still read `~/.claude.json` or `~/.codex/config.toml` and call the port. Reading those
    files is already a credential read the permission rules leave to a person (`permission/category/paths.rs`,
    `holds_credentials`), and a sandboxed Codex without network can't reach the port. This is unchanged from today's
    discovery file.
- **Browsers.** `guard.rs` already refuses an `Origin` that is not loopback. A fixed port makes DNS rebinding easier
  to aim, so the guard also checks `Host` (`127.0.0.1:<port>` or `localhost:<port>` only), as the MCP specification
  recommends. The bearer token is still required either way.
- **Port squatting.** With a fixed port, a program that binds `47821` while Orchester is not running receives the
  token from clients that try to connect. On a single-user computer such a program already runs as the person. On a
  shared computer another user could do it. Settings shows *port taken* whenever the app starts and finds it held.
  Accepted risk for v1; noted in `docs/stable-build.md`.

## Migration

1. **Installed app:** the first start of a build with this feature generates the token, listens on `47821` and
   writes no discovery file. The old `%LOCALAPPDATA%\orchester-app-mcp\<pid>.json` files disappear as the old
   processes end. A stale file of a killed old process is removed by the next start in discovery mode (existing
   cleanup).
2. **QA copies and tests:** `tools/qa-app.ps1` sets `ORCHESTER_APP_MCP_DISCOVERY` to `.tmp/qa/<task>/mcp/` and prints
   that path. The runs' `call_run_app` relay already looks for discovery under the worktree's `.tmp/`
   (`docs/mcp-verification-client.md`) and only needs the new folder. `tools/lurq-mcp-client.py` takes a discovery
   folder argument (or reads the variable) besides the pid. Test harnesses pass `DiscoveryFolder::At` as today.
3. **Docs:** `docs/stable-build.md` › *The new app's MCP server* is rewritten for the stable address, the secret
   store and *Connect an agent*. `docs/mcp-verification-client.md` keeps its QA flow with the new folder.
4. **The person's existing registrations:** none exist for the new app. The old example
   `claude mcp add … orchester …` in `docs/stable-build.md` is for the deprecated desktop's domain server. A person
   who registered that under the name `orchester` re-runs the new command (Claude Code refuses a duplicate name;
   `claude mcp remove orchester` first). Settings › *Connect an agent* says so in its check line.

## Decisions

Decided 2026-10-13 (user):

- Default port 47821. Debug builds and QA copies keep today's free port and discovery file on each start.
- Codex connects through an environment variable: `codex mcp add … --bearer-token-env-var ORCHESTER_APP_MCP_TOKEN`
  with the variable set for the user (`setx` on Windows, `launchctl setenv` on macOS). The `config.toml` entry with
  `http_headers` is the alternative, not the default.
- After *Copy*, the clipboard is cleared after 60 s if it still holds the copied text.
- Header-helper commands are not in v1.
- Naming the program that holds a taken port comes later.
- Still open: whether *Agents connected* is kept only in memory since the app started (recommended below). The
  person did not accept that as recommended.

## Open decisions

1. **Default port.** `47821`: unassigned by IANA and outside the common dev-server ranges. Recommendation: `47821`.
   The person can change it.
2. **Debug builds.** Recommendation: a debug build without the variable keeps today's free port, per-start token and
   user-folder discovery file, so `tools/run.ps1 app` never takes the installed app's port and design verification
   keeps working unchanged. The alternative, a stable address in debug builds too, needs a second default port.
3. **Codex default.** Decided: the variable (`--bearer-token-env-var` + `setx` / `launchctl setenv`); the
   `config.toml` entry is the alternative.
4. **No token in client configuration (later).** Both CLIs accept a headers helper (Claude `headersHelper`, Codex
   `http_headers_helper`). With it, an `orchester-app mcp-token` command could print the header from the secret store
   at connect time, and the token would leave the client config files. It needs both helpers' runtime behaviour
   verified, and on Windows anything running as the person could run that command too, so the gain is small.
   Recommendation: not in v1; revisit with the server work.
5. **Name the program holding a taken port.** Possible (Windows `GetExtendedTcpTable` behind an unsafe FFI module;
   macOS `proc_pidfdinfo` or `lsof`), and it helps when the dev build holds the port. Recommendation: v1 says
   *another program*; add the name later if it bites.
6. **Connected-client list.** Recommendation: in memory, since start, with the client's own name and version marked
   as such. No history in the database.
7. **Clearing the clipboard after 60 s.** Decided: yes, if the clipboard still holds the copied text. If lurq can't
   read the clipboard to compare, that has to be added; the copy line promises it.
