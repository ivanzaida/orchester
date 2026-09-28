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
| `-WithMcp` | Include the embedded MCP server, so agents can drive the installed app. |
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
| `--with-mcp` | Include the embedded MCP server, so agents can drive the installed app. |
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

