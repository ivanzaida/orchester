# ORC-588 · Codex commands fail before launch: sandbox setup refresh (issue #69, 2026-11-01)

On Windows, every shell command of a Codex step of orchester-platform OP-143 run #1 (Implement, repository-write,
`E:/sccache` among its writable folders) failed before it started with `helper_unknown_error: setup refresh had
errors`; so did the read-only Review's Git. Codex's file edits worked. Installed: **codex-cli 0.161.0**
(`codex --version`, WinGet package `OpenAI.Codex_Microsoft.Winget.Source_8wekyb3d8bbwe`, whose
`codex-windows-sandbox-setup.exe` is the helper below).

## Evidence

Supplied by the person from `%USERPROFILE%\.codex\.sandbox\` (Orchester's tools refuse that folder as a credential
location; `auth.json` and `.sandbox-secrets` were not read). Paths are kept; nothing in them is a secret.

- `setup_error.json`: `{"code": "helper_unknown_error", "message": "setup refresh had errors"}`.
- `deny_read_acl_state.json`: `{"principals": {}}`.
- `sandbox.2026-10-31.log`, the OP-143 worktree as the command's folder:

  ```
  [2026-10-31 23:53:32.635 codex.exe] setup refresh: spawning …\OpenAI.Codex_Microsoft.Winget.Source_8wekyb3d8bbwe\codex-windows-sandbox-setup.exe (cwd=E:\projects\orchester-platform\.orchester\worktrees\01a193dd-…, payload_len=3308)
  runtime read/execute validation failed: validate runtime read/execute access on C:\Users\…\AppData\Local\OpenAI\Codex\runtimes\cua_node\5c41f77a7900391c\bin\node_repl.exe: open ACL target for root-only update: The process cannot access the file because it is being used by another process. (os error 32)
  setup error: setup refresh had errors
  [… codex.exe] setup refresh: exited with status ExitStatus(ExitStatus(1))
  read-acl-only mode: applying read ACLs
  read ACL run completed
  ```

  The next command, two seconds later, failed the same way.
- `sandbox.2026-10-30.log`, an Orchester worktree: the same stage failed on
  `…\runtimes\cua_node\5c41f77a7900391c\bin\node_modules\@oai\sky\bin\windows\swift\x64\VCRUNTIME140_1.dll` (os error
  32), and the helper exited with 1.
- `codex features list` (0.161.0, the person's configuration): `computer_use  stable  true`, `browser_use  stable
  true`, `code_mode_host  stable  true`.

## Finding

The failing stage is Codex's own: the setup helper's **setup refresh**, in its **runtime read/execute validation**,
opens files of the Codex app's computer-use runtime (`cua_node`) to update their ACLs and gets a sharing violation
(Windows error 32) because another process holds them open. The helper exits with 1, Codex reports
`helper_unknown_error: setup refresh had errors`, and no sandboxed command starts, read-only or writing. The process
that held the files was not identified.

Nothing of Orchester's integration is in that path: the failing files are neither the worktree, nor a writable folder
(`E:/sccache`), nor anything of the environment the runner passes (`RUSTC_WRAPPER`, `SCCACHE_*`); the read ACL run that
follows succeeds. `windows.sandbox=elevated` stays required (without a Windows sandbox Codex declines even read-only
commands, `crates/core/src/connection/invocation/codex.rs`). The blocker is upstream, in Codex's helper.

## What Orchester does

- **Diagnostic.** A command item that did not complete and whose output has a helper code and a setup stage
  (`helper_<code>: <message naming setup>`) is told once per invocation as
  `RunnerProblem::CodexSandboxSetupFailed { code, message }` beside the command's own end
  (`crates/core/src/connection/app_server/sandbox_setup.rs`); it is redacted with every event, and the conversation
  shows it as a problem row that points to Codex's sandbox log. Tests: `cargo test -p orchester-core --test
  codex_sandbox_setup codex_sandbox_setup`.
- **Computer use off.** Every Codex invocation passes `-c features.computer_use=false` (`ISOLATION`): an agent of a run
  never drives the person's desktop, and the failing runtime is computer use's. The chat already had it.

## Not verified

- Whether `features.computer_use=false` keeps the `cua_node` runtime out of the helper's validation, so that commands
  start while the runtime's files are held: checking it needs a sandboxed command under the person's Codex home, which
  was not run (the person did not permit a sandbox reproduction in this run). Until a live run shows commands start,
  the execution fix is upstream-only, and the diagnostic is what Orchester delivers.
- The exact text of a failed command item as App Server sends it was not captured; the diagnostic matches the
  `<code>: <message>` the agent of issue #69 reported, wrapped or not.
