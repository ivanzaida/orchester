Agent Instructions

Orchester is a desktop application for orchestrating coding agents per project: tasks and runs, pipelines, agent
profiles, instructions, tools, secrets and connections, all owned by a project. It is local-only today; a server will
be added later, so domain logic must not depend on the desktop app.

- Product and interaction design: `docs/design/orchester-workspace.md`. Open design issues:
  `docs/evidence/design-consistency-audit-2026-09-23.md`. All project data in the designs is illustrative fixture data;
  never ship it as real data or defaults.
- Design source: `data/orchester.pen`. It is encrypted — read it only through the `pencil` MCP, never Read/Grep.

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
- `crates/desktop` (`orchester-desktop`, binary `orchester`): lurq UI. Screens call into core; business rules do not
  live in components. Theme tokens live in `crates/desktop/src/theme.rs`.
- Add a crate only when a real boundary exists (for example the server or a runner). No empty placeholder crates.

Hard requirements:

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
9. Domain types enforce their invariants through constructors; do not make fields public just to allow raw struct
   literals from other modules.
10. Secrets: a secret value never reaches logs, `Debug`/`Display` output, prompts, variables, copied text, errors,
    test snapshots or the MCP tree. Secret-holding types must redact in `Debug` and must not implement `Display`.
11. Locks: `tokio::sync::{Mutex, RwLock}` when held across `.await`; `parking_lot::{Mutex, RwLock}` for short
    synchronous critical sections. Never `std::sync::Mutex` in new code.
12. Every user-facing string goes through i18n with a key in every shipped locale (exact key and placeholder
    parity), and every color, font, text size, spacing, radius and border comes from a lurq theme role. Design
    values live only in `crates/desktop/src/theme/`. Exempt from i18n: developer logs, identifiers, raw user data.
13. Tests live in a `tests/` folder of the crate they cover, not in large inline modules. Every `cargo test` carries
    a name filter (`cargo test -p <crate> --test <binary> <filter>`); batch edits and run the affected tests once per
    milestone. `cargo check` / `cargo clippy` are fine at any time.
14. Every user-visible UI change is verified against the Pencil design by a separate agent that did not implement
    it, following `.claude/skills/design-verification/SKILL.md`: a Pencil screenshot of the design node and an
    app screenshot of the same state at the same size (via the embedded MCP server), compared region by region.
    The change is not done until that verifier reports no blocking or major mismatches. `cargo check`, unit tests
    and the implementer's own screenshots are not design verification. If a state cannot be reached, say so.
15. Dependencies go into `[workspace.dependencies]` and are pulled with `workspace = true`. Do not add a crate for
    something a few lines of code can do; state why a new dependency is needed.
16. Before reporting work as done, run `powershell -File tools/check.ps1` and report its result. Report which tests
    you ran and state plainly what you did not run.

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
