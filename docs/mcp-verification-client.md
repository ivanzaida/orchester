# Calling an app's embedded MCP inside a run

Agents use `mcp__Orchester__call_run_app(pid, tool, arguments)` (Codex: `call_run_app` of the
Orchester server), instead of running `tools/lurq-mcp-client.py` themselves. In a run, start an isolated
app with `mcp__Orchester__start_qa_app(scenario, fake_agents?, no_runner?)` (Codex: `start_qa_app`):

```json
{"scenario":"decisions","fake_agents":null,"no_runner":null}
```

The runner takes the computer's QA slot (one QA app at a time, an OS lock on
`<user data>/orchester-qa-slots/slot-0.lock` that every runner takes, whatever its project; the OS frees it
when its holder ends), waiting while another run's QA app holds it; the transcript then says *Waiting for
the QA slot*. It runs the worktree's `tools/qa-app.ps1` with `-Name run-<number>-<step>` (Windows PowerShell
on Windows, `pwsh` on macOS), checks that the app it names is this run's and that its discovery file lies in
its QA folder, and answers its pid. `call_run_app` without `pid` then calls that app through its exact
discovery file. A second start in the same turn replaces the app and keeps the slot; when the agent's turn
ends (the step passes, fails, asks, pauses or is stopped), the runner stops the app with everything it
started inside the worktree and frees the slot. A start still waiting when the turn ends gives up. The call
asks a person (or an *Approve similar* rule) first, as it builds and runs the worktree's code outside the
sandbox; auto mode's reviewer never answers it. No lease, ACK or hold notes between runs are needed.

Outside a run, start the copy with `tools/qa-app.ps1` yourself:

```text
powershell -File tools/qa-app.ps1 -Name <task> -Scenario <scenario> [-FakeAgents] [-NoRunner]
```

It starts the copy with `ORCHESTER_APP_MCP_DISCOVERY` set to the worktree's `.tmp/qa/<task>/mcp/`, so the
copy takes a free port and a new token on each start and writes them to `<pid>.json` there (never the
installed app's fixed address or stored token). It prints `pid <pid>`, `mcp <folder>` and
`discovery <file>`. Retain the printed pid and pass that exact pid on every call (leave it out for the QA
app `start_qa_app` started):

```json
{"pid":12345,"tool":"lurq_windows","arguments":{}}
{"pid":12345,"tool":"navigate","arguments":{"route":"tasks","project":"<id>"}}
{"pid":12345,"tool":"lurq_screenshot","arguments":{"window":"main"}}
```

The relay reuses `stop_run_process`'s resolved executable path, process age and protection rules.
Executables outside this run's worktree are refused before any discovery or HTTP call: the person's
installed app and another run's app are never driven. It finds discovery by pid in the per-user
folder or a QA copy's folder in this worktree's `.tmp/qa/<task>/`, validates the pid, app name and
loopback URL, and keeps the discovery credential inside the host. It reads `.tmp/qa/` one level (at most
1024 entries, else it refuses and names the folder) and in each task folder only these exact files: for the
new app, the `mcp/<pid>.json` that `tools/qa-app.ps1` makes; for a QA profile (a copy whose user data folder is
`.tmp/qa/<task>/profile/`), `profile/orchester-app-mcp/<pid>.json`, or the deprecated desktop's
`profile/orchester-mcp/<pid>.json` and `profile/lurq/mcp/<pid>.json`. The rest of `.tmp/` (build output,
scratch checkouts) is never searched, and a file reached through a link is not used. Text and structured
content are returned with the token redacted. PNG images are saved under `.tmp/run-app/`; open the relative
path returned by the tool and cite it as the app screenshot. No Python invocation, discovery read
or sandbox exception is needed.

The new app (`orchester-app`) shares one embedded server for app and lurq tools; the deprecated
desktop (`orchester`) uses the lurq server for `lurq_*` tools and its domain server for other tools.
A QA copy of the new app writes its discovery file to the folder `ORCHESTER_APP_MCP_DISCOVERY` names.
Without it, a debug build of the new app and the deprecated desktop write theirs to the user folder:
`LOCALAPPDATA` on Windows and `~/Library/Application Support` on macOS. The installed new app writes
none: it listens on the fixed address in Settings › Automation (`docs/stable-build.md`), and runs never
drive it. Keep QA profiles under the worktree's `.tmp/` when overriding the app's data location.

Settings › Automation › *Orchester MCP server* as the installed app shows it has a harness of its own, since a QA
copy shows its one line there instead: `cargo run -p orchester-app --example mcp_settings_qa --
[listening|port-taken|off|token-unreadable]`. It opens on Settings with a throwaway data folder and a token in memory
under `.tmp/qa/mcp-settings/`, and prints `pid` and `mcp <folder>`; connect through that folder (its own QA server,
apart from the one Settings shows). States that need a person's click there (the Codex tab, *Rotate the token?*,
the token shown, Copied) cannot be reached through MCP, by design; `tests/settings_mcp` covers them.

Build and copy programs into a run-owned folder before launch so concurrent builds cannot change
the running candidate, and enable its no-focus startup option. Drive only the app you started.
Close it through the embedded close button, or use `mcp__Orchester__stop_run_process(pid)`; a QA app
`start_qa_app` started is stopped by the runner when the turn ends.

What a verification could not check (a state no scenario reaches, a capture that failed) goes into the
step's result as the outcome `unverified` with its `unverified` list of `{check, reason}`: the run waits
for the person without returning to Implement or using a retry (`.claude/skills/design-verification/SKILL.md`).

`tools/lurq-mcp-client.py` remains a manual diagnostic helper. It is not the agent workflow: an
unsandboxed approval would run whatever that worktree script currently says. For a QA copy, pass the
printed folder, or let it read `ORCHESTER_APP_MCP_DISCOVERY` from its own environment:

```text
python tools/lurq-mcp-client.py <pid> app_state --app orchester-app --discovery .tmp/qa/<task>/mcp
```

Without `--discovery` and the variable it looks in the user folder (`orchester-app-mcp` for
`--app orchester-app`, `lurq/mcp` otherwise). It never prints the token. Do not pass discovery
credentials or other secrets as tool arguments, since arguments can appear in run activity.

A separate verifier must still compare Pencil and app screenshots in the same state and size,
and verify that the user's foreground window was not taken.
