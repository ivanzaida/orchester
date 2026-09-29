# Calling an app's embedded MCP inside a run

`tools/lurq-mcp-client.py` calls one lurq MCP tool on the **exact PID** of an app started by the run. It reads that
PID's discovery file in memory, validates the app name and loopback endpoint, and never prints the discovery file or
bearer token. HTTP redirects are rejected so the token cannot be forwarded to a server-supplied URL. The client also
redacts the token if a tool echoes it in a result. Python 3 is required in the agent's environment.
Results are printed as ASCII-safe JSON, so non-ASCII UI text remains available through JSON escapes even in Windows
PowerShell pipes.

Build and copy the desktop and runner to a run-owned app directory before starting them. The shared Cargo target can
be rebuilt by another checkout or feature set between build and launch (D6). Start only that copied desktop with L6's
no-focus option, record its PID, and use the same discovery data directory for the app and client:

- Windows: set `LOCALAPPDATA` for **both child processes** to a writable run-owned directory. The discovery file is
  under `LOCALAPPDATA/lurq/mcp/<pid>.json`.
- macOS: set `HOME` for **both child processes** to a writable run-owned directory. The discovery file is under
  `HOME/Library/Application Support/lurq/mcp/<pid>.json`.

Call a tool with the PID returned when starting that app, for example:

```text
python tools/lurq-mcp-client.py 12345 lurq_windows
python tools/lurq-mcp-client.py 12345 lurq_read_tree --arguments '{"window":"main","max_chars":1000}'
```

On macOS, use `python3` if `python` is unavailable. `--discovery-dir` can point directly to the run-owned `lurq/mcp`
directory when the client's environment differs from the app's. The optional `--app` value defaults to `orchester`.
Do not put secret values in `--arguments`: command lines and tool results may be recorded in run activity. The client
protects the **discovery bearer token**, not arbitrary data returned by an MCP tool. It does not launch or close the
app; the verifier owns only its own PID and must close that process after the check.

The client is a transport helper. D4 still requires a separate verifier to compare Pencil and app screenshots at
the same state and size, and a host-side check that the user's foreground window was not taken.
