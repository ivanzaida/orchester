---
name: design-verification
description: Independent check that an Orchester screen in the running desktop app matches its Pencil design, by comparing a Pencil screenshot of the design node with a screenshot of the app in the same state and size, and reporting every mismatch. Use this skill when you are the verifying agent for a UI change (the implementer must never verify their own work), when asked to "check the app against the design", or before any UI work is reported as done. The implementing agent loads `orchester-ui`/`lurq-ui`; the verifier loads this.
---

# Design verification

A UI change is done only when a **separate agent** — not the one that implemented it — has
compared the running app with the Pencil design and found no unexplained differences. The
implementer is biased towards seeing what they meant to build; the verifier's job is to see
what is actually on screen.

## Inputs

- The screens and states to check, each with its Pencil node id (see the screen table in
  `docs/design/orchester-workspace.md`).
- The branch/worktree with the implementation.

## Procedure

1. **Design screenshot.** Take it from the live design, not from `docs/design/previews/`
   (previews can be stale): `pencil` MCP → `get_editor_state` (with schema, once), then
   `get_screenshot` for the screen node, or `export_nodes` at scale 1 into
   `.tmp/verify/<screen>/design.png`. Never Read or Grep the `.pen` file.
2. **App screenshot in the same state.** Build and run the implementation with
   `cargo run -p orchester-desktop --features mcp`. Find the MCP discovery file of *your*
   process (`%LOCALAPPDATA%\lurq\mcp\<pid>.json`). Resize the window to 1440 × 1020 logical
   with `lurq_resize` (the 32px custom title bar from "Window chrome" in the design doc, the
   1440 × 960 screen frame and the 28px status bar from "Status bar"; composition example
   `cpoal`), drive the app into the designed state
   (`lurq_read_tree`, `lurq_interact`, `lurq_set_value`), wait for idle (`lurq_wait`),
   and capture `lurq_screenshot` to `.tmp/verify/<screen>/app.png`. Use fixture data that
   matches the design's content where the app allows it, so text lengths are comparable.
   Use a throwaway git repository under `.tmp/`, never a real project.
3. **Look at both images** at full size, region by region — do not rely on the element tree
   or on the code. Compare:
   - overall structure: which regions exist, their order, sizes and alignment; anything
     extra (borders, frames, title bars, panels, fields, notices) or missing;
   - spacing: paddings, gaps, column widths, vertical rhythm;
   - typography: size, weight, color, line height, truncation, capitalization;
   - color and surfaces: backgrounds, borders vs hairlines, hover/selected states;
   - components: the same Pencil component used for the same role (button, field, nav item,
     status label …), icons and their sizes;
   - copy: exact text of labels, titles and hints (translations aside);
   - states: disabled/unavailable, empty, error, focus.
4. **Check the window chrome separately**: the top 32px against the platform title bar
   component (`QQmsI` Windows, `V70DN` macOS), the bottom 28px against the status bar
   component (`pmOpt`, states in `keJ9Z`), the rest against the screen frame. There must be
   no native title bar or window border, and nothing else the OS or toolkit adds that the
   design does not show.
5. **Report** a table per screen: region, design, app, severity (`blocking` = visibly a
   different screen or wrong structure; `major` = wrong size/spacing/typography/color on a
   prominent element; `minor` = small offset or subtle color), and a suggested fix. Attach
   both screenshot paths. A difference is acceptable only if it is a documented, agreed
   deviation (e.g. a feature the design shows that is not built yet and is visibly
   unavailable) — list those separately, never silently.
6. The implementer fixes; the verifier repeats steps 2–5 on the new build. Done means the
   verifier reports no `blocking` or `major` findings.

## Rules for the verifier

- Several agents may run Orchester at the same time, and every instance writes its MCP
  discovery file to the same folder. Never pick "the newest" discovery file: take the pid of the
  process you started, open `%LOCALAPPDATA%\lurq\mcp\<that pid>.json`, and before every
  batch of actions confirm the connection still belongs to that pid (`lurq_windows` / the
  discovery file's `pid`). If it does not, stop and reconnect — never act on another instance.
- Run your own copy of the binaries in your own folder when another agent or the user may be
  using the same `target/` directory, so each instance has its own `orchester.db` next to it.

- Do not edit implementation code; report. (Fixing is the implementer's job, so the next
  comparison is independent again.)
- Do not accept "lurq cannot do this" without evidence; name the missing capability so it can
  be added to lurq.
- If a state cannot be reached in the app, say so explicitly — never report it as matching.
- Kill only the processes you started.
