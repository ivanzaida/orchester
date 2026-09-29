# Host foreground check for a verifier app

`tools/foreground-watch.py` samples the host's foreground **PID** without sending input or capturing
pixels. Run it on the host, outside an Orchester agent CLI sandbox. It does not launch, stop or
inspect the contents of any window. Use a fresh run-owned directory under `.tmp/` for each check.

1. Before launching the pinned no-focus verifier app, create `.tmp/verify/<run>/` and start the
   watcher from a host terminal. Keep it running while the app starts and the separate Verify
   agent calls the exact-PID MCP client:

   ```text
   python tools/foreground-watch.py watch --timeline .tmp/verify/<run>/foreground.jsonl --pid-file .tmp/verify/<run>/app.pid --stop-file .tmp/verify/<run>/stop
   ```

2. Immediately after **that run** starts its pinned desktop, atomically write its returned
   decimal PID to `.tmp/verify/<run>/app.pid` (write a sibling temporary file, then rename it).
   The PID file must not exist before `watch` starts and must remain until `watch` ends. The
   watcher records an `app` marker when it first reads the file. Use the app's run-owned
   `LOCALAPPDATA` (Windows) or `HOME` (macOS) for the app and
   `tools/lurq-mcp-client.py`; never read or print the discovery credential. The verifier should
   also call `lurq_windows` and confirm `focused:false` while the app is alive.

3. After the same app PID has exited, create the `stop` file from the host. Wait for the watcher
   command to finish. Then judge its completed timeline against the PID recorded in the marker:

   ```text
   python tools/foreground-watch.py judge --timeline .tmp/verify/<run>/foreground.jsonl
   ```

   `judge` reads the exact app PID from the timeline's `app` marker. It exits 0 for `pass`, 1
   for `fail`, and 2 for `inconclusive`. A pass requires at least one valid foreground sample
   after that marker and says only that the app PID was absent at the **observed instants**.
   Sampling cannot rule out a brief activation between samples. Keep the timeline and the app
   start/exit times with the verification report, so coverage can be checked independently.

The default interval is 200 ms and the maximum watch duration is 30 minutes; both are CLI options.
An observed own PID anywhere in the timeline is a failure even if other samples are missing.
A missing/changed PID file or marker, a null/zero foreground PID, a sampler error, a sampling gap,
a timeout, or an interrupted watcher makes the result inconclusive. The watcher must reach its
stop file for a pass. It records timestamps and PIDs only, not window titles, screenshots, or
credentials.

On Windows the host sampler calls `GetForegroundWindow` and `GetWindowThreadProcessId` through
Python's `ctypes`. On macOS it asks AppKit's `NSWorkspace.frontmostApplication` through the system
`osascript` interpreter. The macOS command has not been run on a Mac; if it fails or returns no
PID, the watcher reports an inconclusive result. A Python 3 interpreter is required on both
platforms, as it already is for the exact-PID MCP client.
