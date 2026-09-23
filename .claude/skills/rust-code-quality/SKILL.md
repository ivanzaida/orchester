---
name: rust-code-quality
description: How to write, split and self-review Rust code in the Orchester workspace so it stays small, honest and readable — file and function size limits, module splitting, error handling, crate boundaries, and the slop patterns to remove before handing work back. Use this skill before writing or editing any `.rs` file in this repo (new feature, bug fix, refactor, test), before reviewing someone else's Rust change, and whenever a file is approaching 600 lines or a function is getting long. Trigger it even for "small" changes — most bloat arrives one small change at a time.
---

# Rust code quality in Orchester

The goal is code a reviewer can read once and trust. Size limits are the mechanical part;
the rest is about not writing code that looks busy but says nothing. `AGENTS.md` holds the
hard rules; this skill is how to meet them.

## Before writing

1. **Read the neighbourhood.** Open the module you are changing and its siblings. Match the
   existing naming, error type, constructor style and comment density. A second way of doing
   the same thing is a defect even if it compiles.
2. **Check sizes.** `wc -l` the files you will touch. At 600+ lines, split first (see below)
   and land the split as its own step before adding behavior.
3. **Decide where the code belongs.**
   - Domain rule, state transition, validation, composition of instructions → `orchester-core`.
   - Rendering, input handling, layout, view state → `orchester-desktop`.
   - If a component needs an `if` that encodes a business rule, the rule belongs in core and
     the component asks core for the answer.
4. **Find the smallest change that fully solves the task.** Not the smallest diff at any
   cost — the smallest *complete* one. Missing error paths are not "smaller".

## While writing

### Shape

- One concept per module; a type and its `impl` blocks live together.
- Prefer a struct with methods over a loose bag of free functions that pass the same
  arguments around.
- Functions do one thing and fit on a screen (clippy `too_many_lines` = 80). If you need a
  comment to separate "phases" inside a function, those phases are functions.
- Early returns over nested `if`/`match` pyramids.
- Enums for closed sets of states, not strings or booleans that can contradict each other.
  Two `bool` fields that cannot both be true are an enum.

### Types and construction

- Newtypes for identifiers (`ProjectId`, `RunId`) instead of bare `String`/`u64` that can be
  swapped by accident.
- Constructors validate invariants and return `Result` when input can be wrong. Fields stay
  private unless the type is a plain data record with no invariants.
- Share immutable strings and data with `Arc<str>` / `Arc<T>` instead of cloning in loops
  or render paths.

### Errors

- Each core module that can fail has its own error enum with actionable variants. Callers
  match on variants, not on message text.
- Add context where it is lost (which project, which step), not at every layer.
- No `unwrap()` in production. `expect` only for invariants, and the message says why the
  invariant holds.
- Never hide failure: no `let _ = fallible()`, no `.ok()` / `.unwrap_or_default()` that turns
  an error into a plausible-looking value, unless a comment explains why that is correct.

### Comments and docs

- Comment *why*: a constraint, a non-obvious contract, a reference to the design doc. Never
  restate what the next line does.
- Public items in core get a one-line doc comment when the name alone does not carry the
  contract.
- No banner/section comments, no changelog-in-comments, no commented-out code, no `TODO`
  without a concrete follow-up the user agreed to.

## Splitting a file

When a file approaches 600 lines, split by meaning, not by line count:

1. List the types and the responsibilities in the file.
2. Move each cohesive group (a type plus its impls, a family of related helpers, a
   sub-feature) into its own module. Turn `foo.rs` into `foo/mod.rs` + `foo/<part>.rs`
   when needed; `mod.rs` keeps re-exports and glue only.
3. Keep the public API unchanged in the split step so the diff is a pure move.
4. Do not create `utils.rs`, `helpers.rs` or `misc.rs` dumping grounds; name modules after
   what they contain.

## Slop to remove before handing back

Read your own diff as a reviewer and delete any of these:

- Wrapper functions that only forward their arguments.
- Traits, generics, builders or config options with a single use and no boundary reason.
- Defensive checks for states the types already rule out.
- Duplicate logic that already exists elsewhere in the workspace — reuse it.
- Placeholder behavior: fake data, `Ok(())` for unimplemented paths, progress not driven by
  real work, UI states that claim success before it happened.
- Debug leftovers: `dbg!`, `println!`, temporary logs, unused imports, `#[allow(...)]` added
  to silence a warning instead of fixing it.
- Comments that narrate the code or describe the change you just made.
- Changes outside the requested scope — report them instead.

## Tests

- Tests go in `crates/<crate>/tests/`, grouped by feature (`tests/instructions.rs`), and
  exercise behavior through the public API.
- Test the rule, not the implementation: inputs and observable outputs, including the error
  paths the task touched.
- Run only the affected tests, by name: `cargo test -p orchester-core --test instructions
  resolve_order`.
- UI behavior is verified in the running app (see the `lurq-ui` skill and `AGENTS.md`), not
  by unit tests alone.

## Finish

Run `powershell -File tools/check.ps1` (formatting, clippy with warnings denied, file-size
limit). In the report, list what you ran, what passed, and what you did not run.
