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
- `orchester.exe` is a console program today, so a console window with its log opens next to
  the window. Closing that console closes Orchester.

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
  second app cannot start another runner for it. The second app shows the generic *Could not
  start the runner* notice ("The runner could not be started. The log has the details."); *Open
  runner log* shows that another runner is already running for this project. A runner keeps
  running while its app shows another project, so quit the other app before opening the project
  in this one, then use *Restart runner*.
- **Schema versions.** A newer build migrates a project database when it opens it, and an older
  build then refuses that project ("database schema version … is newer than this build
  supports"). Opening a real project with the dev build can therefore lock the stable build out
  of it until you install a build that includes the migration. Do not open real projects with a
  dev build of an unmerged branch that adds a migration: that schema version may never exist on
  `master`.

## macOS

There is no install script for macOS yet. The desktop's runner integration is Windows-only (see
`AGENTS.md`, requirement 0), and the application database next to the executable is not writable
inside a `.app` bundle. Once the desktop builds and runs on macOS, the same approach applies
without a bundle: build with `--target-dir "$HOME/Library/Application Support/Orchester/build"`
and copy `orchester` and `orchester-runner` into `~/Library/Application Support/Orchester/app`.
Copy each program to a new file and move it into place rather than overwriting it: macOS can
kill a program whose file was changed in place, because it caches the code signature.
