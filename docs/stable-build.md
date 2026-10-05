# Using a stable build while developing Orchester

`tools/run.ps1` runs the dev build from `target/debug`, which agents rebuild while they work on
Orchester. For daily use, install a release build in its own directory instead; agent builds
never touch it.

## Install and update (Windows)

```powershell
powershell -File tools/install.ps1
```

- Builds `orchester-desktop` and `orchester-runner` in release into
  `%LOCALAPPDATA%\Orchester\build` (not the repository's `target/`) and copies `orchester.exe`
  and `orchester-runner.exe` into `%LOCALAPPDATA%\Orchester\app`. Fonts, icons and locales are
  compiled into the executables; at runtime they need only the Visual C++ runtime, which the
  MSVC toolchain has already installed.
- It installs what the checkout holds, uncommitted changes included, and prints the commit.
  Install from an up-to-date `master` with a clean working tree.
- To update, close the installed Orchester and run the script again. While it runs from the
  install directory (the desktop or one of its runners), the script refuses and changes nothing;
  it never stops Orchester itself.
- The release build of `orchester.exe` is a Windows program without a console, so starting it
  from Explorer or the Start menu opens only its window. Its log goes to a file (see
  [Logs](#logs)). Dev builds (`tools/run.ps1`) keep the console and print the log there too.

Options:

| Option | Effect |
| --- | --- |
| `-InstallDir <dir>` | Install somewhere else. |
| `-BuildDir <dir>` | Build somewhere else. |
| `-ImportDevData` | Copy the dev app database (see below). |
| `-StartMenuShortcut` | Add *Orchester* to your Start menu. |

## Install and update (macOS)

On a Mac with the Rust toolchain (`rustup`, which picks up `rust-toolchain.toml`) and the Xcode
command line tools (`xcode-select --install`, for the C compiler, `lipo` and `codesign`):

```sh
tools/install-macos.sh
```

- Builds `orchester-desktop` and `orchester-runner` in release into
  `~/Library/Application Support/Orchester/build` (not the repository's `target/`), packages them
  with `tools/package-macos.sh` into `Orchester.app` and moves it to `~/Applications/Orchester.app`.
  Start it from Finder, Launchpad or Spotlight, or with `open ~/Applications/Orchester.app`.
- The bundle is `Contents/MacOS/orchester` (the desktop), `Contents/MacOS/orchester-runner` (the
  desktop starts it from its own folder), `Contents/Resources/orchester.icns` and
  `Contents/Info.plist` (identifier `dev.orchester.desktop`, version from `Cargo.toml`, high
  resolution capable, macOS 11 or later).
- As on Windows, it installs what the checkout holds, prints the commit, and refuses while the
  installed app or one of its runners is running; it never stops Orchester itself. The new bundle
  replaces the old one as a whole (macOS can kill a program whose file was changed in place,
  because it caches the code signature).

Options:

| Option | Effect |
| --- | --- |
| `--install-dir <dir>` | Install somewhere else. |
| `--build-dir <dir>` | Build somewhere else. |
| `--universal` | One app for Apple silicon and Intel (`rustup target add aarch64-apple-darwin x86_64-apple-darwin` first). |
| `--import-dev-data` | Copy the dev app database (see [Data](#data)). |

`tools/package-macos.sh` alone builds `Orchester.app` into `target/macos/` (`--help` lists its
options). A development build runs without a bundle: `cargo build -p orchester-runner && cargo
run -p orchester-desktop` (what `tools/run.ps1` does on Windows).

**Signing.** The bundle is signed ad hoc (`codesign -s -`): no Apple Developer identity, and it
is **not notarized**. A build made on the Mac it runs on has no quarantine flag, so Gatekeeper
lets it start. A copy downloaded or sent from another Mac is quarantined and refused; build it on
that Mac instead (or remove the flag with `xattr -dr com.apple.quarantine Orchester.app` if you
trust the copy). Every build has a new ad hoc signature, so after an update macOS may ask once
more before Orchester reads the project secrets it keeps in the login keychain; choose *Always
Allow*.

**Programs from Finder.** An app started from Finder gets a minimal `PATH`
(`/usr/bin:/bin:/usr/sbin:/sbin`). Orchester therefore looks for the agent CLIs and `gh`, and
starts every program, the runner included, with the usual install folders appended to it:
`/opt/homebrew/bin`, `/usr/local/bin`, `/opt/local/bin`, `~/.local/bin`, `~/.claude/local`,
`~/.npm-global/bin`, `~/.volta/bin`, `~/.bun/bin`, `~/Library/pnpm`, `~/.cargo/bin`, the `bin` of
npm's `prefix` in `~/.npmrc` and nvm's Node.js versions (newest first), each only if it exists; a
program's own folder is appended for it too, so an npm-installed CLI finds the Node.js beside it.
Your shell's start-up files are not read. A CLI installed elsewhere can be chosen as a file in
*Connections*; a tool an agent needs from elsewhere is not found when the app was started from
Finder (`open` starts it the same way). Starting the program itself from a terminal,
`~/Applications/Orchester.app/Contents/MacOS/orchester`, passes that terminal's `PATH` on.

## Embedded MCP server

Every build, debug and release, on Windows and macOS, includes two MCP servers. Both listen on
`127.0.0.1` only, on a free port the OS picks (never a fixed one), and both need a bearer token on
every request, which the app writes, with the port, to a discovery file of its process in your
user profile and removes when the app closes. Orchester never shows, copies or logs a token.

- **Orchester's MCP server** is how automation controls Orchester, the coordinator agent that
  develops Orchester through Orchester included. Its tools work on data and go through the same
  rules as the screens: `list_projects`, `list_tasks`, `get_task`, `create_task` (saves a draft),
  `update_task`, `run_task` (queues and starts a draft, starts a queued task, or runs a task again
  after its latest run ended, as the app allows), `get_run`, `pause_run`, `resume_run` (continues an
  interrupted run), `cancel_run`, `list_approvals`, `decide_approval` and `list_pipelines`. A modal
  open in the window does not stand in their way. A refusal comes back as a tool error with a stable
  `code` (`run_active`, `task_incomplete`, `automation_not_allowed`, a runner rejection such as
  `run_not_running`…) and the app's own explanation. Its discovery file is
  `%LOCALAPPDATA%\orchester-mcp\<pid>.json` on Windows and
  `~/Library/Application Support/orchester-mcp/<pid>.json` on macOS. Only you can read it: on
  Windows it gets an access list of its own that admits only you, on macOS it is `0600`. It holds `url`
  (`http://127.0.0.1:<port>/mcp`, streamable HTTP), `port` and `token`. To connect a client, read
  the file of the running app's pid and send `Authorization: Bearer <token>`, e.g.
  `claude mcp add --transport http orchester http://127.0.0.1:<port>/mcp --header "Authorization: Bearer <token>"`.
  When the app starts it removes the files of Orchester processes that were killed and could not
  remove theirs (each running process holds a lock on `<pid>.lock` beside its file). The server
  refuses a request without the token, and one that a web page sends (an `Origin` other than
  loopback); it serves at most 32 connections, each with a 10-second deadline for a request's
  headers and for idling between requests.
- **lurq's MCP server** is for looking at the window. In a release build, the installed app, it only
  reads: screenshots, the element tree, inspecting and finding elements, windows, waiting and logs
  (`lurq_screenshot`, `lurq_read_tree`, `lurq_inspect`, `lurq_find`, `lurq_windows`, `lurq_wait`,
  `lurq_logs`); it refuses `lurq_interact`, `lurq_act`, `lurq_set_value`, `lurq_resize` and menu
  activation, so it cannot press a button or flip a switch. A debug build (`tools/run.ps1`), which
  design verification drives, also takes synthetic input. The build decides
  (`cfg(debug_assertions)`, `embedded_mcp::McpAccess`). Its discovery file is
  `%LOCALAPPDATA%\lurq\mcp\<pid>.json` on Windows (the folder admits you, SYSTEM and
  administrators) and `~/Library/Application Support/lurq/mcp/<pid>.json` on macOS.

The status bar shows *MCP* with the address of Orchester's server; its hint names both endpoints and
where the token is. If Orchester's server cannot start, the status bar shows *MCP unavailable* and
the app's log has the reason (see [Logs](#logs)).

**Approvals.** Automation answers waiting tool requests only while *Let automation answer approvals*
is on. It is an application setting, off by default, which only a person changes, in the window:
click the MCP item in the status bar and turn it on or off in its popover (no tool of Orchester's MCP
server changes it, and lurq's server of the installed app cannot press it). While it is off,
`decide_approval` is refused (`automation_not_allowed`). While it is on, a client may deny any
request, and approve once only what auto-approval would approve: a request auto-approval leaves to a
person (an unreadable or destructive command, a write outside the worktree, a credential read, an
install, a network command) is refused (`person_only`, with the reason), as are the wider answers
(`automation_wider`). The run records automation's answers as such: the activity reads *approved by
automation* or *denied by automation*, and the run's Approvals count them apart from yours.

What this means: any program running as your user can read the discovery files and control the app's
data through Orchester's server as you could: create and run tasks, whose agents work in your
projects, and pause or cancel runs. Through lurq's server it can read what the window shows; in a
debug build it can also click and type. Other users' programs can connect to the ports but cannot
read the tokens, so the servers refuse them; other machines cannot connect at all. Secret values stay
masked on screen, in the element tree, and in every tool result. A program running as you can
already read your files and start the same CLIs, so the servers add no access such a program lacks;
they make the app scriptable. Answering approvals, the one thing they would add, waits for your
setting and stays within what auto-approval allows.

## Data

The application database (`orchester.db`, the list of known projects and the session log) lives
next to the executable on Windows. The installed app therefore starts with its own, empty
project list, separate from the dev build's `target\debug\orchester.db`. On macOS the installed
`Orchester.app` keeps it in `~/Library/Application Support/Orchester/` (nothing inside an app
bundle is writable); a dev build outside a bundle keeps it next to its executable, as on Windows.

`-ImportDevData` (`--import-dev-data` on macOS) copies the dev database (with its `-wal` file, if
any) from `target/debug` of the checkout the script runs from into the installed app's place. It
is refused when the installed app already has a database, which it never overwrites, and while
the dev build is running. Once the installed app has started it has a database, so import before
the first start.

Project data is not copied: each project keeps its database in its repository's `.orchester/`,
and both builds open the same one.

- **One app per project at a time.** A project's runner holds `.orchester/runner.lock`, so a
  second app cannot start another runner for it. The second app shows *Could not start the
  runner* with "Another Orchester window is already running this project's runner. Close that
  window, then restart the runner." and does not retry on its own. A runner keeps running while
  its app shows another project, so quit the other app, then use *Restart runner*. (The runner
  reports the held lock to the app through its exit code.)
- **Schema versions.** A newer build migrates a project database when it opens it, and an older
  build then refuses that project ("database schema version … is newer than this build
  supports"). Opening a real project with the dev build can therefore lock the stable build out
  of it until you install a build that includes the migration. Do not open real projects with a
  dev build of an unmerged branch that adds a migration: that schema version may never exist on
  `master`.

## Logs

- **The app's log** is `logs\orchester-<YYYY-MM-DD>.log` next to the executable (UTC date); the
  installed macOS app writes it to `~/Library/Logs/Orchester/` (Console.app lists it under *Log
  Reports*). So the installed app and the dev build (`target\debug\logs\`) keep separate logs.
  A session that runs past midnight continues in the next day's file; files older than 14 days are deleted
  when the app starts and when a new day's file begins. Nothing else in `logs\` is touched. A
  panic is logged there as well. `RUST_LOG` sets the level as usual (default `info`).
- **A project's runner log** is `.orchester\logs\runner-<YYYY-MM-DD>.log` in the project's
  repository, shared by both builds; *Open runner log* opens it. The app's log also has the
  runner's lines, as the app received them.

