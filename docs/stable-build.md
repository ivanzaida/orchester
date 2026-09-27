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

## Data

The application database (`orchester.db`, the list of known projects and the session log) lives
next to the executable. The installed app therefore starts with its own, empty project list,
separate from the dev build's `target\debug\orchester.db`.

`-ImportDevData` copies the dev database (with its `-wal` file, if any) from `target\debug` of
the checkout the script runs from into the install directory. It is refused when the install
directory already has a database, which it never overwrites, and while the dev build is running.
Once the installed app has started it has a database, so import before the first start.

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

- **The app's log** is `logs\orchester-<YYYY-MM-DD>.log` next to the executable (UTC date), so
  the installed app and the dev build (`target\debug\logs\`) keep separate logs. A session
  that runs past midnight continues in the next day's file; files older than 14 days are deleted
  when the app starts and when a new day's file begins. Nothing else in `logs\` is touched. A
  panic is logged there as well. `RUST_LOG` sets the level as usual (default `info`).
- **A project's runner log** is `.orchester\logs\runner-<YYYY-MM-DD>.log` in the project's
  repository, shared by both builds; *Open runner log* opens it. The app's log also has the
  runner's lines, as the app received them.

## macOS

There is no install script for macOS yet. The desktop's runner integration is Windows-only (see
`AGENTS.md`, requirement 0), and the application database and the app's `logs` folder next to the
executable are not writable inside a `.app` bundle (without a log file the app logs to the console
only). Once the desktop builds and runs on macOS, the same approach applies
without a bundle: build with `--target-dir "$HOME/Library/Application Support/Orchester/build"`
and copy `orchester` and `orchester-runner` into `~/Library/Application Support/Orchester/app`.
Copy each program to a new file and move it into place rather than overwriting it: macOS can
kill a program whose file was changed in place, because it caches the code signature.
