# Calling an app's embedded MCP inside a run

Agents use `mcp__Orchester__call_run_app(pid, tool, arguments)` (Codex: `call_run_app` of the
Orchester server), instead of running `tools/lurq-mcp-client.py` themselves. Start an isolated app
with `tools/qa-app.ps1`, retain its printed pid, and pass that exact pid on every call:

```json
{"pid":12345,"tool":"lurq_windows","arguments":{}}
{"pid":12345,"tool":"navigate","arguments":{"route":"tasks","project":"<id>"}}
{"pid":12345,"tool":"lurq_screenshot","arguments":{"window":"main"}}
```

The relay reuses `stop_run_process`'s resolved executable path, process age and protection rules.
Executables outside this run's worktree are refused before any discovery or HTTP call: the person's
installed app and another run's app are never driven. It finds discovery by pid in the per-user
folder or an isolated QA profile under this worktree's `.tmp/`, validates the pid, app name and
loopback URL, and keeps the discovery credential inside the host. Text and structured content are
returned with the token redacted. PNG images are saved under `.tmp/run-app/`; open the relative
path returned by the tool and cite it as the app screenshot. No Python invocation, discovery read
or sandbox exception is needed.

The new app (`orchester-app`) shares one embedded server for app and lurq tools; the deprecated
desktop (`orchester`) uses the lurq server for `lurq_*` tools and its domain server for other tools.
Discovery uses `LOCALAPPDATA` on Windows and `~/Library/Application Support` on macOS.
Keep QA profiles under the worktree's `.tmp/` when overriding the app's data location.

Build and copy programs into a run-owned folder before launch so concurrent builds cannot change
the running candidate, and enable its no-focus startup option. Drive only the app you started.
Close it through the embedded close button, or use `mcp__Orchester__stop_run_process(pid)`.

`tools/lurq-mcp-client.py` remains a manual diagnostic helper. It is not the agent workflow: an
unsandboxed approval would run whatever that worktree script currently says. Do not pass discovery
credentials or other secrets as tool arguments, since arguments can appear in run activity.

A separate verifier must still compare Pencil and app screenshots in the same state and size,
and verify that the user's foreground window was not taken.
