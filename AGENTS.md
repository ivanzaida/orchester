Agent Instructions

Orchester is a desktop application for orchestrating coding agents per project: tasks and runs, pipelines, agent
profiles, instructions, tools, secrets and connections, all owned by a project. It is local-only today; a server will
be added later, so domain logic must not depend on the desktop app.

- Product and interaction design: `docs/design/orchester-workspace.md`. Open design issues:
  `docs/evidence/design-consistency-audit-2026-09-23.md`. All project data in the designs is illustrative fixture data;
  never ship it as real data or defaults.
- Work plan and backlog: `docs/plan.md`. Take tasks from it and move them to its *Done* list when finished.
- Design source: `data/orchester.pen`; the new app's (`crates/desktop-app`) is `data/orchester2.pen`. Both are
  encrypted — read them only through the `pencil` MCP, never Read/Grep.

Cross-agent skill layout:

- Repository skills are authored and committed only under `.claude/skills/`.
- `.agents/skills` is a local Windows junction to `.claude/skills` so Codex and Claude load the same files. It is
  git-ignored; run `powershell -File tools/setup-agent-skills.ps1` to recreate it. Never edit skills through the
  junction path or add wrappers there.
- At the start of a Codex turn, if `.agents/skills` is missing, run that script. If the skill catalog was already
  initialized, read the matching `.claude/skills/<name>/SKILL.md` directly.
- Skill files carry YAML frontmatter for Claude Code's loader; other agents ignore it and read the body.

Workspace layout:

- `crates/core` (`orchester-core`): domain model and rules. No UI, windowing, rendering, HTTP, or database client
  dependencies. Persistence, process execution and providers are reached through traits defined here, so the desktop
  app and the future server share the same core.
- `crates/workspace` (`orchester-workspace`): the local application layer both desktop apps share — the session and
  projects, the runners' client side, project data as screens and automation read and change it, the domain MCP
  tools, and the words of its `Message`s (`assets/locales/`, loaded under the namespace `workspace`). No UI: lurq is
  only an optional dependency for empty DevTools markers (feature `devtools-inspect`, `src/inspect.rs`).
- `crates/desktop` (`orchester-desktop`, binary `orchester`): the old lurq UI, deprecated and frozen since
  2026-10-15. It is never updated again: no fixes, no features, no ports of new logic, no QA, no tests for new work.
  Read it only to look up code and behaviour. New work never changes `crates/desktop`; if a change elsewhere would
  stop it from compiling, report that to the person instead of updating it.
- `crates/desktop-app` (`orchester-app`, binary `orchester-app`): the new lurq app in the `orchester2.pen` design.
  Theme roles live in `crates/desktop-app/src/theme/`, strings in `crates/desktop-app/assets/locales/`. It keeps its
  settings in the application database (`orchester.db`, located by `orchester_store::location`) and runs one MCP
  server of its own that hosts lurq's window tools and the app's tools. The installed app listens on the fixed
  address set in Settings › Automation (default `http://127.0.0.1:47821/mcp`, token in the OS secret store, no
  discovery file); a debug build writes a per-start discovery file `%LOCALAPPDATA%\orchester-app-mcp\<pid>.json`,
  and a QA copy writes it to the folder `ORCHESTER_APP_MCP_DISCOVERY` names (`tools/qa-app.ps1`:
  `.tmp/qa/<task>/mcp/`); see `docs/stable-build.md`. Run it with `powershell -File tools/run.ps1 app`.
- `crates/runner` (`orchester-runner`): one process per open project, started by the desktop from the desktop's
  own directory (inside `Orchester.app` both are in `Contents/MacOS`). Run the app with
  `powershell -File tools/run.ps1` (extra arguments go to `cargo run`, e.g. `--features devtools`); plain
  `cargo run -p orchester-desktop` does not build the runner, and the app then shows the runner as unable to start.
  Every build runs two loopback MCP servers (free port, token in a per-user discovery file; see
  `docs/stable-build.md`): Orchester's domain server (`%LOCALAPPDATA%\orchester-mcp\<pid>.json`; tasks, runs,
  approvals, pipelines; the status bar shows its port) for controlling the app, and lurq's UI server
  (`%LOCALAPPDATA%\lurq\mcp\<pid>.json`) for screenshots and the element tree, which only reads in release builds
  and also drives the UI in debug builds (design verification).
  On macOS: `cargo build -p orchester-runner && cargo run -p orchester-desktop` (the new app:
  `cargo build -p orchester-runner && cargo run -p orchester-app`); `tools/package-macos.sh` builds
  `Orchester.app` and `Orchester App.app` and `tools/install-macos.sh` installs both (`docs/stable-build.md`).
- `crates/exec` (`orchester-exec`): runs external programs (git, gh, agent CLIs) for the desktop and the runner,
  implementing core's `CommandRunner`. Platform details (hidden console window, process groups and kill, the macOS
  program search path) stay behind `cfg` here.
- Add a crate only when a real boundary exists (for example the server or a runner). No empty placeholder crates.

Hard requirements:

0. Orchester ships on Windows AND macOS. New code must build and behave on both: platform-specific code lives
   behind `cfg(target_os = …)` in small, named modules with an equivalent implementation for the other platform (or
   an explicit, visible "not supported on this platform yet" state — never a silent gap). Do not add Windows-only
   dependencies without their macOS counterpart. The macOS side (Unix socket transport and peer check, process groups,
   program search path, Library data location, `.app` bundle) is checked with `cargo check`/`clippy --target
   aarch64-apple-darwin` on Windows until it has been run on a Mac. Known gaps today: a macOS process group does not
   hold what leaves it on purpose (`setsid`), and agents outlive a runner that is killed with SIGKILL or aborts (no
   kill-on-close Job Object); CLIs and tools outside the usual install folders are not found when the app is started
   from Finder (the user's shell `PATH` is not read); the `.app` is signed ad hoc and not notarized.
1. Before writing or reviewing Rust code, read `.claude/skills/rust-code-quality/SKILL.md` and follow it.
2. Before touching the visual layer (screen, panel, row, field, dialog, control, interaction, user-facing text,
   layout/text/scroll defect), read `.claude/skills/lurq-ui/SKILL.md` with its `references/`, and
   `.claude/skills/orchester-ui/SKILL.md` (i18n keys and theme roles; no text or style literals in UI code).
3. Rust source files must stay under 600 lines. Before adding code to an existing `.rs` file, check its size; if it
   is already 600+ lines, split it by struct, responsibility or closely related type group first. If the requested
   scope forbids the split, report the needed split instead of growing the file. Never create a new 600+ line file;
   isolate generated code or large static tables in clearly named modules.
4. Functions stay small enough to read at once: clippy's `too_many_lines` threshold (80) and
   `cognitive_complexity` threshold (25) are enforced as warnings, and warnings fail the check.
5. Never go outside the requested scope. Report unrelated issues or tempting improvements separately instead of
   implementing them without approval.
6. No stubs that pretend to work: no `todo!`/`unimplemented!`, silent no-ops, hardcoded fake results, fake or
   frozen progress, or success shown for work that did not happen. If something is not implemented, it is visibly
   unavailable or returns an explicit error.
7. Errors are typed and propagated. No `unwrap()` in production code; `expect("why this cannot fail")` only for
   true invariants. Never discard a `Result` with `let _ =` or `.ok()` unless the comment says why losing the error
   is correct.
8. No speculative code: no trait with a single implementation unless it is a real boundary (storage, runner,
   provider, clock), no generic parameters or config knobs "for later", no dead code, no commented-out code.
9. `unsafe` is denied workspace-wide. It is allowed only for FFI without a safe wrapper, in a dedicated module that
   starts with `#![allow(unsafe_code)]`, explains why at the top, keeps each `unsafe` block minimal and precedes it
   with a `// SAFETY:` comment stating the invariants (today: `crates/ipc/src/pipe_owner.rs`,
   `crates/desktop-app/src/system_icon/windows.rs`, `crates/desktop-app/src/startup/front/windows.rs`,
   `crates/desktop-app/src/mcp/bind/windows.rs`,
   `crates/agent/src/files/entry/windows.rs`, `crates/core/src/permission/remote_drive.rs`,
   `crates/exec/src/run_processes/windows.rs`, `crates/exec/src/run_processes/macos.rs`,
   `crates/exec/src/run_processes/macos/identity.rs`, and the tests `crates/desktop-app/tests/system_icon.rs`,
   `crates/exec/tests/support/process_window.rs`, `crates/exec/tests/support/process_term.rs` and
   `crates/exec/tests/support/console_children.rs`).
10. Domain types enforce their invariants through constructors; do not make fields public just to allow raw struct
   literals from other modules.
11. Secrets: a secret value never reaches logs, `Debug`/`Display` output, prompts, variables, copied text, errors,
    test snapshots or the MCP tree. Secret-holding types must redact in `Debug` and must not implement `Display`.
12. Locks: `tokio::sync::{Mutex, RwLock}` when held across `.await`; `parking_lot::{Mutex, RwLock}` for short
    synchronous critical sections. Never `std::sync::Mutex` in new code.
13. Every user-facing string goes through i18n with a key in every shipped locale (exact key and placeholder
    parity), and every color, font, text size, spacing, radius and border comes from a lurq theme role. Design
    values live only in each app's theme module (`crates/desktop/src/theme/`, `crates/desktop-app/src/theme/`).
    Exempt from i18n: developer logs, identifiers, raw user data.
14. Tests live in a `tests/` folder of the crate they cover, not in large inline modules. Every `cargo test` carries
    a name filter (`cargo test -p <crate> --test <binary> <filter>`); batch edits and run the affected tests once per
    milestone. `cargo check` / `cargo clippy` are fine at any time.
15. Every user-visible UI change is verified against the Pencil design by a separate agent that did not implement
    it, following `.claude/skills/design-verification/SKILL.md`: a Pencil screenshot of the design node and an
    app screenshot of the same state at the same size (via the embedded MCP server), compared region by region.
    The change is not done until that verifier reports no blocking or major mismatches. `cargo check`, unit tests
    and the implementer's own screenshots are not design verification. If a state cannot be reached, say so.
16. Never send input to the desktop globally (`SendKeys`, global mouse/keyboard injection) and never capture the
    whole screen: other windows belong to the user. Drive the app through its embedded MCP server
    (`lurq_interact`, `lurq_set_value`, `lurq_screenshot`); a native dialog of your own process may be driven only
    by targeting that dialog window explicitly. Stop and report if that is not possible.
17. Dependencies go into `[workspace.dependencies]` and are pulled with `workspace = true`. Do not add a crate for
    something a few lines of code can do; state why a new dependency is needed.
18. Before reporting work as done, run `powershell -File tools/check.ps1` and report its result. Report which tests
    you ran and state plainly what you did not run.

Stable build:

- The user runs an installed release build for daily use (`tools/install.ps1`, `docs/stable-build.md`), installed in
  `E:\projects\orchester\` (it holds its own `orchester.db`; the script's default `%LOCALAPPDATA%\Orchester\` is
  no longer used). Never write to, build into or delete from that install, never stop or kill its processes, and run
  `tools/install.ps1` only when asked, into `.tmp/` unless told otherwise (the user's install:
  `-InstallDir E:\projects\orchester`). Stop only Orchester processes you started yourself, by their pid.

Scratch files:

- Put throwaway artifacts (one-off scripts, dumps, captures) in the git-ignored `.tmp/<topic>/` folder, not in OS
  temp dirs or the source tree.

Worktrees:

- Create git worktrees only inside this repository, at `.tmp/worktrees/<agent>-<name>`, where `<agent>` is `codex` or
  `claude` depending on who works in it and `<name>` is a short kebab-case task name. Example:
  `git worktree add .tmp/worktrees/claude-instruction-preview -b <branch>`.
- Never create worktrees outside the repository, in OS temp dirs, or in tool-default locations such as
  `.claude/worktrees/`.
- Remove a worktree with `git worktree remove` once its work is merged or abandoned.

Commits:

- Only commit when asked. Conventional messages: `feat(desktop): ...`, `fix(core): ...`, `refactor(...)`,
  `docs: ...`, `chore: ...`. One logical change per commit.
