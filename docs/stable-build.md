# Using a stable build while developing Orchester

`tools/run.ps1` runs the dev build from `target/debug`, which agents rebuild while they work on
Orchester. For daily use, install a release build in its own directory instead; agent builds
never touch it: build one from the checkout (below), or install a release that CI built (see
[Releases from CI](#releases-from-ci)).

An install holds the app (the package `orchester-app`, the program `orchester`) and the runner it
starts from its own folder, both from the same build. The deprecated app (`crates/desktop`,
`orchester-desktop`) is no longer built.

**Installs from before the program's rename** (2026-10-24) hold the app as `orchester-app.exe`
(*Orchester App*, `Orchester App.app` on macOS) beside the deprecated app's `orchester.exe`
(`Orchester.app`). Installing a newer build there replaces the deprecated app's `orchester.exe`
(`Orchester.app`) with the app and leaves `orchester-app.exe` (`Orchester App.app`), which is then
from an older build: do not start it, since it would open the install's database with that older
build, and delete it yourself, with its Start menu shortcut *Orchester App* if you made one. The
scripts never delete it; they refuse to install while it runs, and warn while it is there.

## Install and update (Windows)

**Back up before the first start of a newer build.** With the app closed, copy aside the
application database `orchester.db` (with its `-wal` file, if any) and each project's
`.orchester\orchester.db` (with its `-wal`) in its repository. A newer build migrates both kinds
when it opens them, and an older build then refuses them, so only these copies let you go back.
The script reminds you when the install folder already has a database.

```powershell
powershell -File tools/install.ps1
```

- Builds `orchester-app` and `orchester-runner` in one release build into
  `%LOCALAPPDATA%\Orchester\build` (not the repository's `target/`) and copies `orchester.exe`
  and `orchester-runner.exe` into `%LOCALAPPDATA%\Orchester\app`. Fonts, icons and locales are
  compiled into the executables; at runtime they need only the Visual C++ runtime, which the MSVC
  toolchain has already installed (the programs CI releases have it linked in).
- To install somewhere else, name the folder, e.g.
  `powershell -File tools/install.ps1 -InstallDir E:\projects\orchester`. An existing install
  there is updated in place: its `orchester.db` stays (see [Data](#data) before the first start of
  a newer build).
- It installs what the checkout holds, uncommitted changes included, and prints the commit.
  Install from an up-to-date `master` with a clean working tree.
- To update, close the installed app and run the script again. While anything runs from the
  install directory (the app, `orchester-app.exe` of an older build, or a runner), the script
  refuses and changes nothing; it never stops Orchester itself.
- The release build of `orchester.exe` is a Windows program without a console, so starting it
  from Explorer or the Start menu opens only its window. Its log goes to a file (see
  [Logs](#logs)). Dev builds (`tools/run.ps1`) keep the console and print the log there too.

Options:

| Option | Effect |
| --- | --- |
| `-InstallDir <dir>` | Install somewhere else. |
| `-BuildDir <dir>` | Build somewhere else. |
| `-ImportDevData` | Copy the dev app database (see below). |
| `-StartMenuShortcut` | Add *Orchester* to your Start menu, with the program's icon. |

## Install and update (macOS)

**Back up before the first start of a newer build.** With the app closed, copy aside the
application database `~/Library/Application Support/Orchester/orchester.db` (with its `-wal` file,
if any) and each project's `.orchester/orchester.db` (with its `-wal`) in its repository. A newer
build migrates both kinds when it opens them, and an older build then refuses them, so only these
copies let you go back. The script reminds you when that folder already has a database.

On a Mac with the Rust toolchain (`rustup`, which picks up `rust-toolchain.toml`) and the Xcode
command line tools (`xcode-select --install`, for the C compiler, `lipo` and `codesign`):

```sh
tools/install-macos.sh
```

- Builds `orchester-app` and `orchester-runner` in one release build into
  `~/Library/Application Support/Orchester/build` (not the repository's `target/`), packages them
  with `tools/package-macos.sh` into `Orchester.app`, and moves it to `~/Applications`. Start it
  from Finder, Launchpad or Spotlight, or with `open "$HOME/Applications/Orchester.app"`.
- The bundle is the program `Contents/MacOS/orchester`, `Contents/MacOS/orchester-runner` (the
  app starts it from its own folder), `Contents/Resources/orchester.icns` and
  `Contents/Info.plist` (identifier `dev.orchester.app`, the app's as before the rename; version
  from `crates/desktop-app/Cargo.toml`, high resolution capable, macOS 11 or later). It keeps its
  data in the Library folder (see [Data](#data)).
- As on Windows, it installs what the checkout holds, prints the commit, and refuses while the
  installed app (or `Orchester App.app` of an older build) or one of its runners is running; it
  never stops Orchester itself. The bundle is copied beside the old one first, then replaces it
  as a whole (macOS can kill a program whose file was changed in place, because it caches the
  code signature).

Options:

| Option | Effect |
| --- | --- |
| `--install-dir <dir>` | Install somewhere else. |
| `--build-dir <dir>` | Build somewhere else. |
| `--universal` | One app for Apple silicon and Intel (`rustup target add aarch64-apple-darwin x86_64-apple-darwin` first). |
| `--import-dev-data` | Copy the dev app database (see [Data](#data)). |

`tools/package-macos.sh` alone builds the bundle into `target/macos/` (`--help` lists its
options). A development build runs without a bundle: `cargo build -p orchester-runner && cargo
run -p orchester-app` (what `tools/run.ps1` does on Windows).

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

## Releases from CI

GitHub Actions builds the Windows app and publishes it as a GitHub Release
(`.github/workflows/app-release.yml`).

**To release**, raise `version` in `[package]` of `crates/desktop-app/Cargo.toml` (the app's own
version, e.g. `0.2.0` to `0.2.1`; a version with a suffix such as `0.3.0-rc.1` is a pre-release)
and get that change onto `master`. The push runs the workflow, which releases when neither a tag
nor a release `app-v<version>` exists yet; any other change to that file, or a run again, releases
nothing. The workspace's `[workspace.package]` version is the team server's (`server-v*` tags,
`docs/server-hosting.md`): it never releases the app, and the app's version never releases the
server. *Run workflow* on `master` releases the same way.

**What it builds**, on a Windows runner: `cargo build --release --locked -p orchester-app -p
orchester-runner` as the installed app (`ORCHESTER_APP_INSTALLED_BUILD=1`, as `tools/install.ps1`
builds it: the MCP server on its fixed address, the token in Credential Manager), with the C
runtime linked in so the programs start without the Visual C++ Redistributable. The release
`app-v<version>`, tagged on the pushed commit, has:

- `Orchester-<version>-setup.exe`, the installer (Inno Setup, `tools/installer/orchester.iss`):
  for the current user, without administrator rights, into `%LOCALAPPDATA%\Programs\Orchester`
  (another folder can be chosen on the first install; an upgrade keeps it), with the Start menu
  shortcut *Orchester*, an optional desktop shortcut and an uninstaller. There is no install for
  all users: the app writes its database beside itself, which *Program Files* does not allow.
- `Orchester-<version>-windows-x64.zip`, the portable app: a folder `Orchester-<version>` with
  `orchester.exe`, `orchester-runner.exe` and a `README.txt`. `orchester.exe` is not released on
  its own: it starts the runner from beside itself and cannot open a project without it.
- `SHA256SUMS.txt`, the checksums of both.

**Your data on install, upgrade and uninstall.** The app keeps `orchester.db` (with its `-wal`
and `-shm` files), `orchester.lock`, `orchester.lock.holder` and `logs\` in its install folder.
The installer writes only the two programs and its uninstaller there: an upgrade replaces the
programs and leaves the rest, and the uninstaller removes only what it installed (and the app's
notification registration under `HKCU\Software\Classes\AppUserModelId\Orchester.App`, which the
app writes again on its next start), so the database, the logs and the folder stay, and it says
so. The MCP token in Credential Manager stays with them. Back up before the first start of a newer
version, as for any install (above). While Orchester or one of its runners runs from the install
folder, the installer and the uninstaller ask you to close it (*Retry*) or stop (*Cancel*); they
never close it themselves. The installer's folder is not the one `tools/install.ps1` uses; point
it at an existing install only if that is the one you want to update (see *Installs from before
the program's rename* above).

**Not signed.** Neither the programs nor the installer carry a code signature, so SmartScreen
warns (*More info*, then *Run anyway*); no secret is needed. Signing would be one step before
packing, with a certificate in the repository's secrets.

**Dry run.** *Actions › App release › Run workflow*, on any branch, with *dry_run* ticked: it
builds and packs both files and keeps them, with the checksums, as the run's artifact for 7 days,
and releases nothing. Every run costs CI minutes.

macOS is not built by CI yet; `tools/package-macos.sh` makes `Orchester.app` on a Mac and would be
the base of a later job on a macOS runner (a bundle signed ad hoc is refused by Gatekeeper on other
Macs, see *Signing* above).

## Embedded MCP server

The deprecated desktop (`orchester-desktop`, no longer built; this part stays for its existing
installs) included two MCP servers in every build, debug and release, on Windows and macOS. Both listen on `127.0.0.1` only, on a free port the OS picks (never a
fixed one), and both need a bearer token on every request, which the app writes, with the port, to
a discovery file of its process in your user profile and removes when the app closes. It never
shows, copies or logs a token. The new app runs one server, and the installed new app listens on a
fixed address instead (see *The new app's MCP server* below).

- **Orchester's MCP server** is how automation controls Orchester, the coordinator agent that
  develops Orchester through Orchester included. Its tools work on data and go through the same
  rules as the screens: `list_projects`, `list_tasks`, `get_task`, `create_task` (saves a draft),
  `update_task`, `run_task` (queues and starts a draft, starts a queued task, or runs a task again
  after its latest run ended, as the app allows), `return_to_draft` (a queued task whose run never
  started, as the task list's *Return to drafts* takes it back: only one its group holds for good;
  refused with `not_queued`, `run_recorded` or `not_held`), `get_run`, `pause_run`, `resume_run`
  (continues an interrupted run), `cancel_run`, `send_message` (a message to the agents of a task's
  latest run, as the Conversation composer sends it in the run's state: it answers, follows up,
  continues a failed or stuck run, resumes a paused one or runs an ended task again; it waits for a
  limit the run stopped at to be raised, answering `awaits_limit`; refused with `no_agent_reads` or
  `not_latest_run`), `continue_run` (a run stopped at a limit, with a
  `choice` its task page offers for how it stopped: `raise_limits` with `budget_tokens` and
  `time_limit_minutes` after its token budget or time limit, `raise_step_budget` with `budget_tokens`
  after a step's budget, `resume` after the daily budget, `allow_retry` or `return_to` with `step` and
  `note` after a step used up its retries or broke again; every value is given, read with the page's
  rules; refused with `not_stopped_at_limit`, `choice_not_offered`, `invalid_limit`, `group_budget`,
  `step_not_offered` or `task_blocked`), `set_run_auto_mode`, `get_auto_mode` and `set_auto_mode` (auto mode,
  see below), `remove_worktree` (an ended run's worktree, as the task page's *Remove worktree* removes it;
  refused with `run_not_ended`, `merge_running`, `uncommitted_changes` or `already_removed`; `get_task` and
  `get_run` show each run's worktree as `present`, `removed` or `none`), `list_approvals`,
  `decide_approval`, `decide_plan` (the plan a gated step waits with, decided as the task page's plan card
  decides it: `approve` with the `version` it is for and no note, only while *Let automation answer
  approvals* is on (refused with `automation_not_allowed` otherwise), or `request_changes` with a `note` the
  step's agent resumes with and an optional `version`, which needs no such permission; the run records the
  decision as automation's and the plan card says so, *Approved by automation* or *Changes requested by
  automation*, as does a plan's change requested through `send_message`; `get_run`'s `awaiting.plan` shows the waiting plan's
  version, step, whole `text` and an `excerpt`, and `list_approvals`' `plans` the same without the whole
  text, so a client approves only a plan it read in full through `get_run`; both show in the waiting plan's
  `earlier` the decisions on its step's earlier versions, and `get_run`'s `plans` every decided plan of the
  run with its step: `decision` (`approved` or `changes_requested`), `note`, `decided_by` (`person` or
  `automation`; decisions recorded before it was kept read `person`) and `decided_at`; refused with `run_not_found`,
  `no_plan_waiting`, `run_paused`, `plan_already_decided`, `plan_version_changed` (another version waits now,
  named in the reason), `version_required`, `invalid_version`, `blank_note`, `note_not_taken` or a runner
  rejection such as `run_not_running`, `shutting_down` or `runner_unavailable`), `list_pipelines`, `duplicate_pipeline` (copies a
  pipeline's latest published version under a new name, optionally with other published profiles on its
  agent steps, and publishes it as v1, as New pipeline, the editor and Publish do), `set_code_index`
  (*Use code index* in Project settings, off until a person turns it on; `list_projects` shows each
  project's setting) and
  `update_project_limits` (Project settings › Limits: the token budgets, the default time limit,
  *Agents working at once*, which the project's runner applies at once, and *QA apps at once*, which
  applies from the next `start_qa_app`; invalid limits are refused with `invalid_limits` and the tab's
  own words; `list_projects` shows each project's limits). A modal
  open in the window does not stand in their way, and they work in the background without disturbing
  whoever uses the window: only the changed project's data is read again, the screens showing it
  update in place (no page switches, no form is reset; an Edit task form or a settings section with a
  change of the person's keeps it and says quietly that the data changed elsewhere, with *Reload*),
  and a tool's failure is only in its result, never a failure in the status bar or a mark on a tab. A refusal comes back as a tool error with a stable
  `code` (`run_active`, `task_incomplete`, `automation_not_allowed`, a runner rejection such as
  `run_not_running`…) and the app's own explanation. Its discovery file is
  `%LOCALAPPDATA%\orchester-mcp\<pid>.json` on Windows and
  `~/Library/Application Support/orchester-mcp/<pid>.json` on macOS. Only you can read it: on
  Windows it gets an access list of its own that admits only you, on macOS it is `0600`. It holds `url`
  (`http://127.0.0.1:<port>/mcp`, streamable HTTP), `port` and `token`. To connect a client, read
  the file of the running app's pid and send `Authorization: Bearer <token>`, e.g.
  `claude mcp add --transport http orchester http://127.0.0.1:<port>/mcp --header "Authorization: Bearer <token>"`.
  The new app's command registers its server under the same name `orchester`, and Claude Code
  refuses a name it already has: if you registered the deprecated desktop's server this way, run
  `claude mcp remove orchester` before you connect the new app. When the app starts it removes the files of Orchester processes that were killed and could not
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
request, with a `message` of its own or without, and approve once only what auto-approval would approve: a
request auto-approval leaves to a
person (an unreadable or destructive command, a write outside the worktree, a credential read, an
install, a network command) is refused (`person_only`, with the reason), as are the wider answers
(`automation_wider`). The run records automation's answers as such: the activity reads *approved by
automation* or *denied by automation*, and the run's Approvals count them apart from yours. A denial's
message (at most 2000 characters) reaches the agent as the denial's reason, *Denied by automation through
Orchester's MCP server:* and the words; the request's card keeps them.

**Decision consent in the new app.** The window and domain tools use `orchester_workspace::decisions::DecisionKind`.
*Let automation answer approvals* covers approval answers, question answers, every plan decision (including
requesting changes), and retry, return, stop and Accept decisions on a waiting run. It does not require
*Let automation change agent permissions*: that separate grant still governs permission setup. Without decision
consent, synthetic window input and the corresponding domain tools refuse the decision. With consent, they record
automation as the decider; a person's own press records the person. Approval answers retain their existing restrictions
on approving risky requests or granting wider approval. Free-text answers and continuations keep their sender too.
A native acceptance of an Orchestrator proposal is the person's decision; automatic answers still require consent.

**The new app's MCP server.** `orchester-app` runs one MCP server of its own instead of two. It listens on
`127.0.0.1` only and needs a bearer token on every request; where it listens and where its token is kept depend on
how the app was started (below). It hosts lurq's window tools, the app's
own (`app_state`, `navigate`, `set_theme`, `hold_reads` in debug builds) and Orchester's domain tools:
every tool of Orchester's MCP server above, and the setup tools that only this app serves, which set a
project up end to end without its configuration screens: `add_project` and `get_project`;
`set_project_environment` and `set_repository_agent_files` (Project settings; limits through
`update_project_limits`); `list_connections`, `add_cli_connection` (Codex or Claude, checked before it is
saved), `check_connection` and `list_models`; `add_mcp_server`, `update_mcp_server`, `set_mcp_server_enabled`
and `remove_mcp_server` (Connections › MCP servers); `list_profiles`, `get_profile`, `save_profile_draft` and
`publish_profile`; `get_pipeline`, `create_pipeline` (from the starter, empty, or a copy),
`save_pipeline_draft` and `publish_pipeline`; `list_tools`, `get_tool`, `save_tool_draft` and `publish_tool`
(command tools, which pipeline steps call; HTTP and database tools are listed by kind and description only,
and no tool is removed or archived in this version); `list_instructions`, `save_instruction_draft` and
`publish_instruction`; `get_changes` and `merge_run` (the Summary's *Merge into main*). Each checks its
input with the rules of the screen it stands for and takes only what that screen offers; no tool takes or
returns a secret's value. Tools that read are in lurq's Observe scope. Tools that change data are in a
`data` scope that is always granted, apart from window control (`navigate`, `set_theme`, lurq's input),
which a release build grants only when its settings or `ORCHESTER_APP_MCP_WINDOW_CONTROL=on` allow it.

| Started as | Address | Token | Discovery file |
| --- | --- | --- | --- |
| The installed app: built by `tools/install.ps1`, `tools/package-macos.sh` (`Orchester.app`) or the release workflow (Releases from CI) | Fixed, from Settings › Automation; `http://127.0.0.1:47821/mcp` until you change it | Kept in the OS secret store | None |
| Any other build with `ORCHESTER_APP_MCP_DISCOVERY=<folder>` (QA copies, test harnesses) | A free port on each start | A new one on each start, in memory only | `<folder>/<pid>.json` |
| Any other build without that variable: debug (`tools/run.ps1 app`), `cargo run --release`, an example | A free port on each start | A new one on each start | `%LOCALAPPDATA%\orchester-app-mcp\<pid>.json`, macOS `~/Library/Application Support/orchester-app-mcp/<pid>.json` |

What makes the installed app is how it was built, not where it runs: `tools/install.ps1` and
`tools/package-macos.sh` set `ORCHESTER_APP_INSTALLED_BUILD=1` while they compile it, and nothing else does. So a
build of your own, release or debug, never takes the installed app's port or leaves a credential in your secret
store. The installed app refuses `ORCHESTER_APP_MCP_DISCOVERY`: started with it, it runs without its MCP server and
the log says why, so a program that starts it cannot get a token for your data in a file it can read.

A discovery file holds `url` (`http://127.0.0.1:<port>/mcp`), `port` and `token`, with the same rules as the
deprecated desktop's above: only you can read it, a `<pid>.lock` beside it, removed when the app closes.
`tools/qa-app.ps1` sets the variable to `.tmp/qa/<task>/mcp/` in the worktree and prints that folder. Other builds
therefore never take the installed app's port, and they can run beside it. The runs' `call_run_app` relay drives
only an app whose program is inside the run's worktree, whatever discovery file it finds.

**The installed app's address.** Settings › Automation › *Orchester MCP server*. *Let agents connect to Orchester*
is on by default. Off, nothing listens, the status bar shows *MCP off*, and the token stays stored for when you turn
it on again. *Address* shows the address with *Copy*. Enter another port (1024 to 65535) and choose *Use port*: the
server moves there at once, without restarting the app, and agents registered with the old port need registering
again. If another program holds the port, the server does not listen and does not move to another port, because an
agent registered with the fixed address would reach nothing, or reach that program. Settings says *Port … taken*
and the status bar *MCP port … taken*; choose another port, then register your agents again. Orchester does not name
the program that holds the port yet. The port and the switch are kept in that installation's `orchester.db`.

**The token.** The installed app makes it on its first start and keeps it in the OS secret store, never in a file,
the database, a log or the MCP tree: on Windows as a generic credential in Credential Manager, target
`orchester-app-mcp/<installation id>`; on macOS in the login keychain, service `orchester-app-mcp`, account
`<installation id>`. The installation id is a random id in that installation's `orchester.db`, so a dev build and
the install never share a token. As with project secrets, macOS may ask once more after an update before the app
reads it (see *Signing* above). In Settings, *Show* shows it for 30 seconds at most, and while it shows the server
refuses screenshots and the window's text (`token_shown`). *Copy* puts it on the clipboard and clears the clipboard
after 60 seconds, or when the app closes before then, if it still holds what Orchester put there. *Rotate token* asks in place, stores a new token and
ends the open sessions: register every agent again, and for Codex set `ORCHESTER_APP_MCP_TOKEN` again. If the
secret store refuses the new token, the old one stays in use. Show, Copy, Rotate token, *Use port* and the switch
take only a person's click in the window; a change brought by automation is not taken. If the token cannot be read,
the server stays off, the status bar shows *MCP unavailable*, and Settings says why, with *Try again*.

**Connect an agent.** Settings › Automation › *Connect an agent* gives the command for each CLI with the current
address and the token masked; its *Copy* puts the whole command, token included, on the clipboard (cleared after 60
seconds as above). Register the server once; the agent then has its tools in every session as
`mcp__orchester__…`. The name is `orchester` in lower case, never `Orchester`: that is the name of the relay that runs
give their agents, and in Codex a server of yours with that name would merge into it.

Claude Code:

```text
claude mcp add --transport http --scope user orchester http://127.0.0.1:47821/mcp --header "Authorization: Bearer <token>"
```

`--scope user` makes it available in every folder; `--scope local` (Claude Code's default) only in the folder you
run it in. Both keep it in `~/.claude.json`, outside any repository. Never use `--scope project`: it writes
`.mcp.json` into the repository, and the token would be committed. Check with `claude mcp get orchester`.

Codex reads the token from a variable, not from its configuration:

```text
codex mcp add orchester --url http://127.0.0.1:47821/mcp --bearer-token-env-var ORCHESTER_APP_MCP_TOKEN
```

Then set the variable for your user. On Windows, `setx ORCHESTER_APP_MCP_TOKEN <token>`; it reaches programs started
after it, so restart Codex, your terminal or your IDE. On macOS, `launchctl setenv ORCHESTER_APP_MCP_TOKEN <token>`
for apps started from the Dock or Finder (until you sign out), and `export ORCHESTER_APP_MCP_TOKEN=<token>` in
`~/.zshrc` for terminals. Check with `codex mcp get orchester`. The alternative, without a variable, is an entry in
`~/.codex/config.toml`:

```toml
[mcp_servers.orchester]
url = "http://127.0.0.1:47821/mcp"
http_headers = { Authorization = "Bearer <token>" }
```

Never put the token in a repository's `.mcp.json` or `AGENTS.md`. *Agents connected* lists the clients that
connected since the app started (or since the token changed), named as each client names itself; nothing of it is
stored.

Agents in runs do not reach this server: Claude Code runs keep `--strict-mcp-config`, Codex runs turn your
`orchester` server off, and neither gets `ORCHESTER_APP_MCP_TOKEN` or `ORCHESTER_APP_MCP_DISCOVERY` (their
environment is cleaned).

The project's Orchestrator chat does not reach it either. Its agent gets a server of its own. The agent is Claude
Code or Codex App Server, started read-only in the project folder by the project's runner for each message (Codex
with no tool of its own that reads or runs anything: `docs/design/orchestrator-toolset.md`, decision 10).

- **One server per turn.** It runs on a free loopback port of the app, with a token made when the message is sent.
  It stops once the turn ends, or at once when the runner never took the message, so a token works only while its
  turn runs.
- **Where the token goes.** The app hands it only to the runner, over its pipe. For Claude Code the runner writes it
  only into the agent's MCP configuration file for the turn, which is removed when the CLI ends. For Codex it goes
  only into the process environment.
- **What it serves.** Part of the domain tools: `list_tasks`, `get_task`, `get_run`, `list_approvals`,
  `list_pipelines`, `create_task`, `update_task`, `run_task`, `pause_run`, `resume_run`, `hold_queue` and
  `release_queue`. They act on the chat's project only, given by its id whatever name the call uses. No window tool.
- **Same checks as here.** Each call is the same tool an MCP client calls here, so it is refused or carried out
  exactly as for such a client: what only a person decides stays `person_only`, and approvals stay with people.
- **`run_task` starts nothing.** It proposes the start, and the person starts it in the conversation.

**What the server checks.** The `Host` of every request must be `127.0.0.1:<port>` or `localhost:<port>`, so a web
page cannot reach the server through DNS rebinding, which a fixed port makes easier to aim. An `Origin` other than
loopback is refused, and the bearer token is required either way.

**Who can read the token.** Any program running as you can read a Credential Manager credential, the files of the
clients you registered (`~/.claude.json`, and `~/.codex/config.toml` with the alternative), the variable
`ORCHESTER_APP_MCP_TOKEN` (every program you start gets it, and on Windows it is also in `HKCU\Environment`) and the
clipboard after *Copy*. That is the same exposure as a discovery file (see *What this means* below). Other users of
the computer can connect to the port but cannot read the token.

**Port squatting.** With a fixed port, a program that listens on it while Orchester is not running receives the
token from the agents that try to connect. On a computer only you use, such a program already runs as you; on a
shared one, another user could do it. Settings shows the port as taken whenever the app starts and finds it held.
This is an accepted risk for now. While Orchester listens, no other socket can share the port: on Windows the server holds it with
`SO_EXCLUSIVEADDRUSE`, and on macOS a listening port is shared only by sockets that all set `SO_REUSEPORT`, which
Orchester's does not.

**Let automation change agent permissions.** Any client with the token may set a project up, so the setup
tools only narrow what agents may do unless a person allows more. Widening it is refused with
`person_only` and the reasons while this application setting is off: a profile's permissions or MCP
servers broader than its latest published version (a new profile: the most restrictive permissions its
CLI enforces and no MCP server) on save or publish, which covers write access, writable folders, broader
commands, network access, no longer asking before tool calls and approving automatically; an agent CLI
whose program is not a copy found on the search path, refused before it runs; any change to the project's
environment variables, removals included, but to `RUST_LOG`, `RUST_BACKTRACE`, `CI`, `NO_COLOR`,
`FORCE_COLOR`, `TZ`, `LANG` and the standard `LC_ALL`, `LC_CTYPE`, `LC_COLLATE`, `LC_MESSAGES`, `LC_MONETARY`,
`LC_NUMERIC` and `LC_TIME` (`orchester_core::permission::widening::harmless_variable`), since a
variable can change what programs load, where they connect, what they trust or where tools keep their
configuration, and removing a person's (a proxy, say) can widen too; approving automatically, which also needs
*Let automation answer approvals*; removing or weakening a pipeline step's approval gate; and command tools, since
a command step runs its program with your rights, network access and no sandbox, on files the agents before it wrote
(scripts, build files, package and git configuration): making one, changing its program
or arguments, letting more exit codes pass, changing any of its variables but the harmless ones above, publishing
its first version or a version that does any of these, and a pipeline step that calls a tool where neither the
draft nor the latest published version calls it under that key; and the project's
MCP servers (`add_mcp_server`, `update_mcp_server`, `set_mcp_server_enabled`): adding one, renaming one (the
answers a person gave in a run, *Approve similar*, name a server's tools by its name), giving one another program,
arguments, starting folder, endpoint or bearer secret, changing any of its variables but the harmless ones above
(removals included), giving any variable a secret's value whatever its name, adding a tool that runs without
asking, and turning one on. These are judged by the fields as given, before anything reads the disk, so a refused
program on a share is never contacted. A server's program, from the window or a tool, is stored as the fixed file
it is, as a command tool's: a name found on the search path or an absolute path, links followed, refused when it
lies inside the project's repository, is relative to the worktree, or names another computer or a device
(`\\server\share`, `\\?\UNC\…`, `\\.\…`, a drive mapped to a share, or a link that leads there), by its text
before it is read. Only the project's active secrets can be named (`NAME@environment`), never their values;
`list_connections` names a variable and whether it is plain text or a secret, but leaves out plain values and the
values of an endpoint's query, where a person may have pasted a token. Dropping a server's bearer secret or exempt
tools, turning it off and removing it (`remove_mcp_server`; refused as `in_use` while a profile's draft or latest
published version chooses it, checked and removed in one transaction) are narrowing. In the window, a save or Turn
on that widens asks in place. With the permission opt-in on, authenticated clients with independently granted
window control may answer the matching permission confirmations; otherwise their window input is refused.
Automatic approvals require the separate approvals opt-in too. Secret replacement and publishing a stale agent
draft remain person-only, irrespective of the permission opt-in. Consent is re-read before forwarded input and
checked again when a confirmation runs; unknown, pending, revoked or failed consent cannot authorize it.
Narrowing is always taken. The setting is off by
default and only a person changes it, in the window, in Settings › What automation may do. No domain tool
changes it, and window input cannot either: while its row is on screen, lurq denies its input tools when a
call runs (also one queued earlier, or one whose answer timed out), the server refuses every window-input
tool of lurq's (clicks and keys by element or by coordinates, values, resizing) with `person_only`, whatever
window control allows, and the switch takes no change that turns it on while an MCP client's input is being
applied, nor any before it was shown for one pass; turning it off is always taken. `navigate` may still
show Settings, and looking at the window stays allowed.

Pipelines follow the same rule. With the setting off, `publish_pipeline` and `duplicate_pipeline` publish
only a version provably no weaker than the pipeline's latest published one: the same steps, wiring,
failure routes, profiles and tools, and each step's approval gate, asking about tool calls, write access,
commands and network the same or stricter. A pipeline's first version and anything else need a person, so
every version a task can run is one a person published, allowed, or could not object to. A version is
weaker also when a step gets more retries, a larger step budget or none, other instructions or another model;
the pipeline's description may change. Accepted behaviour, as the user decided: automation may create a
task on any published pipeline with the setting off, since every published version is one a person
published, allowed, or could not object to. Only moving an existing task to another pipeline
(`update_task`) is a person's decision, refused as `person_only` while the setting is off; a task without a
pipeline may get any published one.
`duplicate_pipeline` is shared with the deprecated desktop's server, which follows the rule too.

Command tools through the tools: `save_tool_draft` takes the program as a name found on the search path (the
tool's own `PATH` variable, else the project's, else the app's; relative folders are skipped) or as the absolute
path of an existing file outside the project's repository, and stores the file it found, so the program itself is
a fixed file; a path inside the repository, also through a link or junction, is refused (run a script through its
interpreter instead). On an existing tool a field left out keeps its value. Arguments go to the program as they
are, without a shell, in the worktree of the step that calls the tool; on Windows it does not start programs from
that worktree by name (`NoDefaultCurrentDirectoryInExePath`). A fixed program is not a sandbox: a command step
runs with your rights, network access and no sandbox, and its program reads the files the agents before it wrote,
such as `tools/check.ps1` itself, `build.rs` or a cargo, npm or git configuration. Approve a tool step only after
agents you would let run code that way. For example, a *Check* step that runs `tools/check.ps1` after *Implement* and returns to it
when the check fails, with *Let automation change agent permissions* on:

```text
save_tool_draft     {"project": "Orchester", "name": "Check",
                     "description": "Runs tools/check.ps1 with the tests of the changed crates.",
                     "program": "powershell",
                     "arguments": ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "tools/check.ps1",
                                   "-Tests", "changed"],
                     "timeout_secs": 10800}
publish_tool        {"project": "Orchester", "tool": "Check"}
save_pipeline_draft {"project": "Orchester", "pipeline": "Orchester change", "steps": [...the steps it has...,
                     {"key": "check", "name": "Check", "tool": "Check", "runs_after": ["implement"],
                      "on_failure": {"return_to": "implement", "retries": 2}}]}
publish_pipeline    {"project": "Orchester", "pipeline": "Orchester change"}
```

`-Tests changed` makes the check run tests as well (`tools/check-tests.ps1`): every test binary of the crates the
branch changed since it left `master` (committed or not), and of the workspace crates that depend on them, one
`cargo test -p <crate> --test <binary>` after the other. orchester-app counts as depending on orchester-runner and
orchester-cloud too, as its tests start their programs, so a change in the runner, the agent or the cloud runs the
orchester-app tests as well. A file outside the crates counts for the crates whose sources include it
(`include_str!`), for example `docs/cloud-hosting.md` for orchester-cloud. Without `-Tests` `tools/check.ps1` runs
no test, so the check agents run by hand stays fast; when clippy fails the tests are skipped, as they would not
build. A failing test listed in `tools/check-tests-known-failures.txt` (each with its reason) is reported and does
not fail the step; any other failing test fails it, as do a binary that fails without naming a test, one that runs
longer than 30 minutes (`-TestTimeoutSeconds`; it is ended with the processes it started), one that runs no test
without being listed there with `(none)`, and tests that do not build. The step's output names each new failure
with what the test printed; the full output of every binary is in the worktree's `.tmp/check-tests/<run>/`.

The base is the merge base with `master` (or `origin/master`, whichever is newer); `-Base <revision>` names another.
A tool step cannot pass its run's base, so a run based on another branch tests the crates changed since `master`:
more than needed, never fewer. Measured on the person's Windows machine with `CARGO_BUILD_JOBS=4` (2026-10-19): a
change in `crates/desktop-app` alone adds about 40 minutes (5 to build when *Implement* built its tests already, 12
from cold, then 36 to run its 67 test binaries), one in `crates/runner` about 50 (its own 10 and orchester-app's),
and one in `crates/core`, which every crate depends on, about 2 hours; hence the timeout of 3 hours. The tests build
with line tables only, as full debug information gave each orchester-app test binary a PDB of about 900 MB; even so
the orchester-app tests take about 35 GB of the worktree's `target/`. On macOS the program is `pwsh` with the same
arguments.

Today only this switch is protected from window input. Person-only actions in editors still to come (a
profile's or a pipeline's Publish, approvals) must likewise refuse synthetic input from MCP clients; lurq
0.43 adds a way to tell it apart from a person's.

What this protects against: an MCP client that holds the server's token and nothing else. Any program that
runs as your user can still change `orchester.db` and the project's `.orchester` data directly; the switch is
not a boundary against those.

**Auto mode.** Modelled on Claude Code's auto mode, and not "approve everything": a separate reviewer
model judges each request the rules leave to the person (rules of the run, Always allow and
auto-approval answer first) in its context: the request as the approval card shows it (secret values
removed), the step's permissions, the task and the agent's last few messages. It approves only what
clearly serves the task and is not destructive, exfiltrating or escaping, and not an install of
anything persistent; everything else still asks the person, with its reason. Hard rules come first, on
every part of a command line Orchester can read, and the reviewer cannot override them: a credential
read, or a write to one, also through a redirect of a part Orchester cannot read; a rewrite of history or a
delete outside the worktree; any push but a plain `git push [-u] origin <the run's branch>` (no option
before `push`, `HEAD` only while the run's branch is checked out, `origin` still pushing where it did as
the run started, nothing else in the line changing git's configuration, remotes or folder); every Codex
escalation; anything outside the step's
permissions (a write in a read-only step, the network or an install while it is off, a write outside the
worktree and its writable folders); Orchester's `run_check` and `start_qa_app`; and a request longer than a review
reads whole (8,000 characters; it is never cut) all ask the person. A part Orchester cannot read to the
end (an unknown program, a script) goes to the reviewer, which is told so, except in a step whose
network is off or that only reads.
In a read-only step a Cargo build, check, test, clippy, doc or bench that writes only its build output (a
target directory in a writable folder outside the worktree, or one in the worktree that git ignores and
tracks nothing in) is no write and goes to the reviewer (2026-10-18); `cargo fmt`, `fix`, `add`, `install`,
`publish`, aliases (`b`, `c`, `t`), what after `--` may write (`--logfile`, `--emit`, `-o`), `--manifest-path` and a
target directory elsewhere still ask. The person accepted (2026-10-18) that such a build runs the worktree's
build scripts, macros, tests and repository Cargo settings under the reviewer's approval alone. *Approve for every run of this
project* on a `start_qa_app` card keeps a rule of the project, by the QA script's git blob, which answers the
tool in every run until the script changes or the rule is revoked in Project settings › Auto mode; only a
person makes it.
A reviewer that fails, takes longer than 60 seconds or
answers with something unreadable asks the person too. The reviewer is any agent connection of the
project with a model of it and an effort, chosen in Project settings › General › Auto mode (the same model
list as an agent profile's) or through the MCP server; until a person chooses, it is the first Claude Code
connection with `claude-haiku-4-5-20251001`, else the first connection a reviewer runs on with its default
model. Claude Code runs in print mode with no tools, no MCP servers and no settings, its instructions as
the system prompt (`--system-prompt-file`); Codex runs `codex -a never exec` in its read-only sandbox with
the user's configuration, rules, shell tool and web search off and the verdict's `--output-schema` (these
flags were not run against a live Codex); both run in an empty folder the runner makes and removes. A
native connection on the Anthropic API gets one Messages request without tools through Orchester's own
client, its key read from the keychain in the runner; a native OpenAI API connection cannot review yet.
What the agent wrote reaches every reviewer only as JSON data on one line between markers it cannot
write, with the step's facts from Orchester before and after it, and the instructions tell it that data
is untrusted; the answer must be the verdict's JSON alone. Without a reviewer (no connection, a chosen one
removed or not supported) auto mode is unavailable and the app says why. The Approvals switch, the
popover and Project settings say which model on which connection reviews, and each recorded review keeps
the model that gave it. A person who answers first stops the review (its process and what it started); so
do the end of the step or run and the runner's exit, and a run whose auto mode is turned off meanwhile
drops the review's result. While the reviewer judges a request, the request waits without asking the person: no approval dialog,
no *Needs you*, and the agent's time keeps counting; only its card in the conversation says *Auto mode is
reviewing* and keeps the answers, so a person may decide before. It asks the person once the reviewer
asks, fails or times out; a hard rule asks at once. Unreadable parts alone are no reason for the reviewer
to ask, and Claude Code reviews without its nonessential startup traffic. Each run has its own *Auto mode* switch, in its Approvals and, as a section of
its own that saves at once apart from Apply, in its permissions popover; *New runs use auto mode* in
Project settings (off by default) turns it on for runs created from then on. The run records each review
with the request: the activity marks *approved by auto mode*, and under the row, the dialog's explanation
and the conversation's card a line of its own says what auto mode did, the reviewer's reason quoted on
one line (cut at 200 characters, line breaks and double quotation marks removed), never inside a mark or
chip. The run's Approvals count *Approved by auto mode* and, once the person decided, *Auto mode asked
you*. `get_auto_mode` reads a project's setting and reviewer; `set_auto_mode` (new runs, reviewer) and
`set_run_auto_mode` change them only while *Let automation answer approvals* is on; `list_approvals`
carries what auto mode did with each request and which model reviewed it.
Not done yet: turning auto mode on while a request waits does not review that request.

What this means: any program running as your user can read the discovery files and the installed new app's
token (see *Who can read the token* above) and control the app's data through Orchester's server as you could: create and run tasks, whose agents work in your
projects, and pause or cancel runs. Through lurq's server it can read what the window shows; in a
debug build it can also click and type. Other users' programs can connect to the ports but cannot
read the tokens, so the servers refuse them; other machines cannot connect at all. Secret values stay
masked on screen, in the element tree, and in every tool result. A program running as you can
already read your files and start the same CLIs, so the servers add no access such a program lacks;
they make the app scriptable. Answering approvals, the one thing they would add, waits for your
setting and stays within what auto-approval allows.

## Data

The application database (`orchester.db`, the list of known projects, the session log and the
app's settings) lives next to the executables on Windows. The install starts with its own, empty
project list, separate from the dev build's `target\debug\orchester.db`. On macOS the installed
bundle keeps it in
`~/Library/Application Support/Orchester/` (nothing inside an app bundle is writable); a dev build
outside a bundle keeps it next to its executable, as on Windows.

- **One app at a time.** The app that opens the database holds `orchester.lock` beside it (who
  holds it is in `orchester.lock.holder`) until it has ended; the system lets go of it also when
  the app crashes or is killed. Another app started on the same data (a second copy, or an app of
  another build left beside it) opens nothing there: the app shows *Orchester is already open* with the
  folder and the other window's process, *Switch to that window* (Windows) and *Close this window*,
  or *Orchester is closing* while the other one is ending; the deprecated app shows *Orchester
  cannot open its database*: "Another Orchester window is using this data folder. Switch to that
  window, or close it and start Orchester again." with the folder. Close the one you are done with, then start the other.
- **The app and its runner come from one build.** `tools/install.ps1` (and
  `tools/install-macos.sh`, the installer and the zip of a release) always installs them together.
  Never copy a program from another build beside them, and do not start an `orchester-app.exe`
  left from before the program's rename (see the top of this page).

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
- **The application database migrates too.** A newer build migrates `orchester.db` when it first
  starts (`orchester-app`'s settings, for example), and an older install then refuses that
  database. Back up both kinds of database before the first start of a newly installed build (see
  the install sections), so the older install can still be restored with its data.
- **What a too-new database looks like to an older build.** The new app shows *Orchester can't
  open its data*: "The database was written by a newer version of Orchester. It has data version
  … ; this build reads up to version …. Nothing was changed." The deprecated app shows *Orchester
  cannot open its database*: "The data was written by a newer version of Orchester." with the
  database's path, instead of its window.
  A project database a newer build migrated is refused the same way when the project opens (see
  *Schema versions* above). Neither changes the file. Install the newer build again,
  or restore the copy you set aside. A build refuses only a schema version it does not know: what
  a newer build writes within a version the older one knows (new kinds of run events, for
  example) is not refused but may be misread, one more reason never to run apps from different
  builds on one database.

## Logs

- **The app's log** is `logs\orchester-app-<YYYY-MM-DD>.log` (the name it had before the program's
  rename; the deprecated app wrote `logs\orchester-<YYYY-MM-DD>.log`), next to the executables (UTC
  date); the installed macOS app writes it to `~/Library/Logs/Orchester/` (Console.app lists it under *Log
  Reports*). So the installed apps and the dev build (`target\debug\logs\`) keep separate logs.
  A session that runs past midnight continues in the next day's file; files older than 14 days are deleted
  when the app starts and when a new day's file begins. Nothing else in `logs\` is touched. A
  panic is logged there as well. `RUST_LOG` sets the level as usual (default `info`).
- **A project's runner log** is `.orchester\logs\runner-<YYYY-MM-DD>.log` in the project's
  repository, shared by both builds; *Open runner log* opens it. The app's log also has the
  runner's lines, as the app received them.

