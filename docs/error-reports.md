# Error reports

When Orchester's desktop app or its runner fails, it can describe the failure in an error report. Reports use the
Sentry protocol, which GlitchTip speaks. This page lists what a report carries, and what is removed before a report
leaves the computer.

Every report from the app and the runner goes through one scrubber first: `ReportScrubber::scrub` in the
`orchester-report` crate (`crates/report`), which is the Sentry client's `before_send`. The scrubber works from
a list of what to keep. Anything not on that list is dropped, including fields that later versions of the protocol
add.

## When reports are sent

The desktop app and its runners send reports, of their panics and of the ERROR lines of their logs, to the GlitchTip
project `desktop` (`orchester_app::error_reports`, `orchester_app::app_log::error_reports`,
`orchester_runner::error_reports`), with the same client options (`orchester_report::client`). The tag `process` says
which sent it: `app` or `runner`.

- **Only ERROR lines.** A WARN, INFO or lower line sends nothing and is no breadcrumb either. The app logs at WARN
  what a person causes or expects (wrong input, a program that is not installed, a network that is down, a file with
  no program to open it), so those are not reported.
- **One report per panic.** A panic is also logged at ERROR, but only the panic itself is reported, with its stack
  trace.
- **A log line's fields** (`error=…`, `path=…`) are added to the message as `name=value` before the scrubber runs, so
  they are cleaned like the rest of the message. An ERROR line's report carries the stack where it was logged.

- **Only while the setting is on.** The app reads it from `orchester.db` as soon as the database is open, right after
  the session starts and before the window opens. A panic before that point is not reported: while the log starts,
  while the window's position is checked, and while the session opens the database.
- **Only the installed app and QA copies.** The installed app reports as environment `production`. A QA copy that
  `tools/qa-app.ps1` starts reports as `qa`, and so does a release build started with `ORCHESTER_APP_MCP_DISCOVERY`.
  Other debug builds, `cargo run --release` and the tests never start the client.
- **The runner reports only when the app says so.** The runner never reads `orchester.db`. While the app's own
  client runs, every runner it starts gets the environment variables `ORCHESTER_ERROR_REPORTS` (the app's environment)
  and `ORCHESTER_ERROR_REPORTS_RELEASE` (the app's release); a runner started without them sends nothing, and the app
  removes them from a runner's environment when it does not report. A runner keeps what it was started with until it
  exits. Debug builds of the runner, a QA copy's among them, and the tests never report.
- **No other data.** The client sends no default personal data (`send_default_pii` is off), no traces and no
  sessions. The release is `orchester@<version>`, the app's version from its `Cargo.toml`, for the runner's reports
  too.
- **Sent before exit.** When the app or a runner exits, it waits up to two seconds for pending reports to leave. The
  panic is still written to the app's log file, and a runner's to its stderr; its log file and stderr stay as they
  are.

## What a report carries

- **What went wrong**: the error message and the type of the error (for a crash, `panic`).
- **Where it happened**: the stack frames. Each frame keeps the function, the module, the source file and the line
  and column. A file inside the Orchester source tree is shown relative to it, for example
  `crates/runner/src/main.rs`. Each frame also keeps the memory address of its instruction.
- **Which program files were loaded** (debug images): for each, its file name without its folder (for example
  `orchester.exe` or `ntdll.dll`), the file name of its debug file (`orchester.pdb`), its debug and code ids, and
  where in memory it was loaded and how big it is. A release build's frames have no names on the person's computer,
  because its PDB is not installed: GlitchTip finds the uploaded debug file by the image's debug id and turns each
  frame's address into its function, file and line.
- **How serious it was**: the level (error, fatal, warning and so on).
- **Which Orchester**: the release (the version) and the environment (for example `production`).
- **Which computer, roughly**: the operating system's name and version, and the CPU architecture (for example
  `x86_64` or `aarch64`).
- **Orchester's own tags**: only the tags Orchester sets itself: `process`, which says whether the app or the runner
  sent the report, and on a frozen window's report `screen` and `stack` (below).
- The report's own id and the time it was made.

## A frozen window

A window that stops responding is no panic, so the app watches for it itself (`orchester_app::hang_reports`). Its
window's thread beats once a second from a timer its event loop runs; a watchdog thread checks the beat every second.

- **When.** The window has given no sign of life for **10 seconds**. A minimized window is not watched (the beat
  carries the minimized state as the window's thread last saw it), and time the computer slept or the OS suspended the
  process is not counted: when the watchdog's own checks are five seconds or more apart, it did not run either, and it
  counts from then. The window is watched from its first beat until it closes; the startup before it and the work
  after it (the runners stopping) are not.
- **Only with the other reports.** The watchdog runs only while the app's client does (above): the installed app or a
  release build a QA harness started, with the setting on. A debug build, `tools/qa-app.ps1`'s QA copy among them,
  and the tests never start it, and without a client it reads no stack and sends nothing.
- **One report per hang**, sent at once (the client's queue is flushed), so it has left even if the person then ends
  the process. Its error type is `ApplicationNotResponding`, its message how long the window has not responded, and
  it carries:
  - the tag `screen`: the kind of screen the window showed, as the app's MCP tools name it (`tasks`, `task`,
    `settings`, …), never which project, task or file it showed;
  - the window's thread's **stack**, as instruction addresses only, read from the watchdog thread: on Windows x86-64
    the thread is suspended for the moment its registers are read and unwound with the loaded modules' unwind tables,
    then resumed. GlitchTip turns the addresses into functions, files and lines with the uploaded PDBs, by the debug
    images the report lists, as for a panic. The tag `stack` is `captured`;
  - on macOS, and on Windows on ARM, no stack yet: the tag `stack` and the message say `not captured on macOS yet`
    (or `on this platform yet`) instead of leaving it out.
- **Its recovery** is a second report, at level info: *the window responds again after N s*, with the tag `screen`.
  It is a report rather than a breadcrumb, because the scrubber removes every breadcrumb (below).
- **No minidump.** A hang report carries no memory dump of the process: a dump holds memory the scrubber cannot clean
  (task text, file contents, tokens), so none is made unless a later decision says otherwise.
- **No deadlock detector.** parking_lot's `deadlock_detection` would find a lock taken twice on one thread, but Cargo
  cannot turn a feature on for debug builds only, and it would slow every lock of the release build, so it is not on.

## What is removed

- The computer's name (`server_name`).
- Everything about the person (`user`): id, email, user name, IP address.
- Everything about a web request (`request`): address, headers, cookies, body.
- Extra data attached to the report (`extra`).
- Every breadcrumb: the trail of earlier events that led up to the failure.
- Tags that Orchester did not set itself.
- Everything else in the system details: the computer's model, memory, disks, time zone, OS build and kernel.
  Other details are removed too: the runtime, the app and the GPU.
- From stack frames: the lines of source code around the failing line, variable values, every memory address but
  the instruction's, and the path of the program file.
- From debug images: the folders of the program file and of its debug file, and the system SDK's details.
- From the error: how it was caught and which thread it happened on. From threads: their names.
- Installed modules, the SDK's details, the culprit, the transaction, the logger name, and the parameters of a log
  message (its text is kept as the message).

## What is changed in the text that stays

The message, the error type, function and module names, file names and Orchester's tags are rewritten:

- **Files in the Orchester source tree** are shown relative to it, even when the tree is inside the home folder.
- **The home folder** becomes `~`, and nothing inside it shows: `C:\Users\ivan\work\acme\notes.txt` becomes
  `~\<path>`, and `/Users/ivan/projects/acme/x.rs` becomes `~/<path>`. A project in the home folder is hidden this
  way.
- **Any other absolute path** becomes `<path>`, for example a project folder such as `D:\work\acme`, including one
  written after a colon or in a `file://` address. A Windows path with spaces (`C:\Program Files\Acme\acme.exe`) is
  hidden as a whole. Folders are matched with `\` and `/` treated alike and with letters in any case, and only by
  their whole name: `C:\Users\ivan.smith` is not the home folder `C:\Users\ivan`, so it becomes `<path>`.
- **Anything shaped like a secret** becomes `<redacted>`:
  - API keys starting with `sk-` (OpenAI, Anthropic), and GitHub tokens (`ghp_`, `gho_`, `ghu_`, `ghs_`, `ghr_`,
    `github_pat_`)
  - the token after `Bearer`
  - any run of 32 or more hex digits, such as a hash or a git commit id
  - any run of 32 or more base64 characters that mixes upper case, lower case and digits

  Short ids such as task ids and UUIDs stay, and so do ordinary words such as `sk-learn`.
