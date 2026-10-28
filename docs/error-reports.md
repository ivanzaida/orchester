# Error reports

When Orchester's desktop app or its runner fails, it can describe the failure in an error report. Reports use the
Sentry protocol, which GlitchTip speaks. Orchester does not send any reports yet. This page lists what a report
will carry once sending is added, and what is removed before a report leaves the computer.

Every report from the app and the runner goes through one scrubber first: `ReportScrubber::scrub` in the
`orchester-report` crate (`crates/report`), which becomes the Sentry client's `before_send`. The scrubber works from
a list of what to keep. Anything not on that list is dropped, including fields that later versions of the protocol
add.

## What a report carries

- **What went wrong**: the error message and the type of the error (for a crash, `panic`).
- **Where it happened**: the stack frames. Each frame keeps the function, the module, the source file and the line
  and column. A file inside the Orchester source tree is shown relative to it, for example
  `crates/runner/src/main.rs`.
- **How serious it was**: the level (error, fatal, warning and so on).
- **Which Orchester**: the release (the version) and the environment (for example `production`).
- **Which computer, roughly**: the operating system's name and version, and the CPU architecture (for example
  `x86_64` or `aarch64`).
- **Orchester's own tags**: only the tags Orchester sets itself. Today that is `process`, which says whether the app
  or the runner sent the report.
- The report's own id and the time it was made.

## What is removed

- The computer's name (`server_name`).
- Everything about the person (`user`): id, email, user name, IP address.
- Everything about a web request (`request`): address, headers, cookies, body.
- Extra data attached to the report (`extra`).
- Every breadcrumb: the trail of earlier events that led up to the failure.
- Tags that Orchester did not set itself.
- Everything else in the system details: the computer's model, memory, disks, time zone, OS build and kernel.
  Other details are removed too: the runtime, the app and the GPU.
- From stack frames: the lines of source code around the failing line, variable values, memory addresses and the
  path of the program file.
- From the error: how it was caught and which thread it happened on. From threads: their names.
- The list of loaded program files (debug images), installed modules, the SDK's details, the culprit, the
  transaction, the logger name, and the parameters of a log message (its text is kept as the message).

## What is changed in the text that stays

The message, the error type, function and module names, file names and Orchester's tags are rewritten:

- **The home folder** becomes `~`: `C:\Users\ivan\AppData\x.log` becomes `~\AppData\x.log`, and
  `/Users/ivan/notes.txt` becomes `~/notes.txt`. Only the home folder itself is replaced. The folders inside it
  stay as written.
- **Files in the Orchester source tree** are shown relative to it.
- **Any other absolute path** becomes `<path>`, for example a project folder such as `D:\work\acme`. A Windows path
  with spaces (`C:\Program Files\Acme\acme.exe`) is hidden as a whole. Folders are matched with `\` and `/` treated
  alike and with letters in any case.
- **Anything shaped like a secret** becomes `<redacted>`:
  - API keys starting with `sk-` (OpenAI, Anthropic), and GitHub tokens (`ghp_`, `gho_`, `ghu_`, `ghs_`, `ghr_`,
    `github_pat_`)
  - the token after `Bearer`
  - any run of 32 or more hex digits, such as a hash or a git commit id
  - any run of 32 or more base64 characters that mixes upper case, lower case and digits

  Short ids such as task ids and UUIDs stay, and so do ordinary words such as `sk-learn`.
