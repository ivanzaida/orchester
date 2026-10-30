# Install, data, platforms and current limits

## 1. Installing

### Windows — release installer

Raising `version` in `crates/desktop-app/Cargo.toml` on `master` signs and uploads a release to Orchester Cloud, where
it is published on Beta, and tags the commit `app-v<version>` (no GitHub Release since 2026-10-31;
`docs/stable-build.md`, "Releases from CI"). It has, at `https://api.orchester.cloud/v1/releases/<version>/files/<name>`:

- `Orchester-<version>-setup.exe` — an installer for the current user, no admin rights. Installs into
  `%LOCALAPPDATA%\Programs\Orchester` by default (another folder can be chosen on the first install), adds a Start menu
  shortcut, an optional desktop shortcut and an uninstaller.
- `Orchester-<version>-windows-x64.zip` — the portable app: `orchester.exe` + `orchester-runner.exe` (the app needs the
  runner beside it).

The installer is **not code-signed**, so SmartScreen warns (*More info* → *Run anyway*). An upgrade replaces the
programs and keeps `orchester.db`, the logs and the MCP token. The installer and uninstaller ask you to close a running
Orchester; they never close it themselves.

### Windows — from source

```bash
powershell -File tools/install.ps1
```

Builds into `%LOCALAPPDATA%\Orchester\build` and installs into `%LOCALAPPDATA%\Orchester\app`. Options:
`-InstallDir`, `-BuildDir`, `-ImportDevData`, `-StartMenuShortcut`. Refuses while anything runs from the install
folder.

### macOS — from source

```bash
tools/install-macos.sh
```

Builds `Orchester.app` into `~/Applications` (options `--install-dir`, `--build-dir`, `--universal`,
`--import-dev-data`). The bundle holds `Contents/MacOS/orchester` and `orchester-runner`, identifier
`dev.orchester.app`, macOS 11 or later. It is **signed ad hoc and not notarized**: build on the Mac itself (a copy
from another Mac is quarantined). After an update macOS may ask again before the app reads the keychain — choose
*Always Allow*. CI does not build macOS yet.

### Development

```bash
powershell -File tools/run.ps1
```

Runs the debug build with its runner (extra arguments go to `cargo run`). Plain `cargo run -p orchester-app` does not
build the runner, and the app then shows the runner as unable to start. QA copies: `tools/qa-app.ps1`.

### Before the first start of a newer build

Back up `orchester.db` (with its `-wal` file) and each project's `.orchester/orchester.db`. A newer build migrates
them, and an older build then refuses them ("Orchester can't open its data").

## 2. What you need

- **git** — projects are git repositories and runs work in git worktrees.
- **An agent CLI** — Claude Code (`claude`) and/or Codex (`codex`), installed and signed in.
- **An Orchester Cloud account** — the app is locked until this computer signs in once.
- Optional: **`gh`** (GitHub CLI) for GitHub detection, pull requests and the Pull requests page; **ripgrep** for the
  code index (installable from Project settings › Context › Code index).

## 3. Data locations

| What | Windows | macOS |
|---|---|---|
| Application database `orchester.db` (projects list, app settings, accounts, plan-limit readings, kept form edits) | Beside `orchester.exe` | `~/Library/Application Support/Orchester/orchester.db` (beside the executable for a development build) |
| App log | `logs\orchester-app-<YYYY-MM-DD>.log` beside the executable (14 days kept) | `~/Library/Logs/Orchester/` |
| Account folders Orchester creates | `%LOCALAPPDATA%\Orchester\accounts\<cli>-<name>` | `~/Library/Application Support/Orchester/accounts/…` |
| Project data | `<repo>\.orchester\` | `<repo>/.orchester/` |
| Runner log | `.orchester\logs\runner-<YYYY-MM-DD>.log` | same |
| Secrets, MCP token, cloud and server tokens | Windows Credential Manager | login keychain |

`.orchester/` holds `orchester.db`, `blobs/`, `worktrees/`, `skills/`, `index/`, `tools/` (the copy of ripgrep),
`attachments/`, `logs/`, `runner.lock` and `step-result.schema.json`. It is excluded from git through
`.git/info/exclude`. A linked shared project may have `orchester.toml` in the repository root (never committed by
Orchester).

**One app per data folder.** The app holds `orchester.lock`; a second copy shows *Orchester is already open*. Each
project's runner holds `.orchester/runner.lock`, so one app runs a project at a time.

## 4. Windows and macOS differences

| | Windows | macOS |
|---|---|---|
| Window controls | Minimize/maximize/close at the right | Traffic lights at the left |
| Back / Forward | Alt+← / Alt+→ | ⌘[ / ⌘] |
| New tab link | Ctrl+click | ⌘-click |
| Notifications | Toasts (AppUserModelID `Orchester.App`); taskbar overlay badge; a toast clicked after the app closed opens nothing | UserNotifications (asks permission once; only from the `.app` bundle); Dock badge |
| *Switch to that window* (already open) | Works | Not available yet |
| Codex *Network off* | Unavailable (the sandbox can't block the network) | Enforced by the sandbox |
| Open in editor | Default program | `open -t` |
| Account sign-in terminal | New PowerShell window | Terminal |
| Programs found from the launcher | Normal `PATH` | An app started from Finder gets a minimal `PATH`; Orchester adds Homebrew, `/usr/local/bin`, `~/.local/bin`, `~/.claude/local`, npm, volta, bun, pnpm, cargo and nvm folders, but doesn't read your shell start-up files. Start `Orchester.app/Contents/MacOS/orchester` from a terminal to pass its `PATH`. |
| Install ripgrep | winget | Homebrew |

## 5. Known gaps

- **macOS**: a process group does not hold what leaves it on purpose (`setsid`); agents can outlive a runner killed
  with SIGKILL; CLIs outside the usual folders aren't found when started from Finder; the `.app` is signed ad hoc and
  not notarized.
- **Windows**: programs and the installer are not code-signed.
- **Fixed MCP port**: a port taken by another program is reported, not avoided.
- **Single window**, no background mode: closing the window stops every runner and agent.
- **System theme** is read at start only.

## 6. Not built yet

The design describes more than this version implements. The following are shown as locked or are absent:

**Tasks and runs**

- Selecting rows in the Tasks **list** (selection and bulk actions exist on the board only); *Run selected* and the
  scheduling review; *Create multiple*.
- A workflow-status picker on the task page or in the form (the board moves tasks between statuses); creating tags in
  the window and *Manage tags* (existing tags are toggled on the board; new ones only over MCP); Blocked / blocked-by
  fields (over MCP only).
- The run's *Approvals* section (changing a run's permissions, or switching it to Manual, in the window — over MCP:
  `set_run_manual`).
- *Always allow…* on approval cards; a per-category approval policy.
- Run artifacts other than step reports (the Summary's *Output* lists the reports); *Ask agent* on the Ready card (use
  *Send follow-up*); the Conversation's step rail, filter and search.
- Deleting a module (modules can only be closed).

**Configuration**

- Presets; pipeline triggers (schedule, GitHub event — runs start only from tasks); the Deliver step's *Pull request*
  mode.
- HTTP, database and MCP-import tool kinds; tool access rules, inputs/outputs and usage log.
- Native API connections (Anthropic API, OpenAI API); adding or editing a GitHub connection; choosing a CLI program
  other than the PATH copy; removing an agent connection.
- MCP server tool discovery and OAuth.
- Importing secrets; instruction variable fallbacks.
- Renaming a project, changing its key or moving its folder.
- Project setup steps 2 and 3; Orchestrator settings' *Actions / Context / Conversations* tabs.

**Shell, pull requests, cloud**

- Tab drag and reorder, tab restore on restart, reopening closed tabs, several windows; Ctrl/⌘+1…9 and
  Ctrl+PageUp/PageDown.
- Pull request *Agent checks*, *Overview*, *Viewed* and linking an unlinked PR to a task.
- Notifications for delivery states.
- Listing your other Orchester Cloud devices (*Other devices* says "Not in this app yet"); member and role editing
  (done in the server admin); *Take*/*Release* controls for shared tasks.
