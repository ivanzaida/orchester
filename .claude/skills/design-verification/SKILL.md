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

- The app: `orchester-app` (`crates/desktop-app`, design `data/orchester2.pen`; see
  "orchester-app" below) or the deprecated `orchester-desktop` (`crates/desktop`, design
  `data/orchester.pen`).
- The screens and states to check, each with its Pencil node id: for orchester-app a frame of
  `data/orchester2.pen` (layers below); for the deprecated desktop see the screen table in
  `docs/design/orchester-workspace.md`.
- The branch/worktree with the implementation.

## Procedure

1. **Design screenshot.** Take it from the live design, not from `docs/design/previews/`
   (previews can be stale): `pencil` MCP → `get_editor_state` (with schema, once), then
   `get_screenshot` for the screen node, or `export_nodes` at scale 1 into
   `.tmp/verify/<screen>/design.png`. Never Read or Grep the `.pen` file, and never edit it.
2. **App screenshot in the same state.** For orchester-app, follow "orchester-app: start,
   connect, reach the state" below. For the deprecated desktop, build and run the implementation with
   `cargo run -p orchester-desktop` (every build includes the embedded MCP server; set
   `ORCHESTER_MCP_START_WITHOUT_FOCUS=1` so the window does not take focus). Find the MCP
   discovery file of *your* process (`%LOCALAPPDATA%\lurq\mcp\<pid>.json`). Resize the window to 1440 × 1020 logical
   with `lurq_resize`, which takes physical pixels: the logical size times the display's scale factor
   (2160 × 1530 at 1.5×) (the 32px custom title bar from "Window chrome" in the design doc, the
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
4. **Check the window chrome separately**: for the deprecated desktop, the top 32px against the
   platform title bar component (`QQmsI` Windows, `V70DN` macOS), the bottom 28px against the
   status bar component (`pmOpt`, states in `keJ9Z`), the rest against the screen frame; for
   orchester-app, the frame's own title bar (tab strip and window buttons), sidebar and status
   bar. There must be no native title bar or window border, and nothing else the OS or toolkit
   adds that the design does not show.
5. **Report** a table per screen: region, design, app, severity (`blocking` = visibly a
   different screen or wrong structure; `major` = wrong size/spacing/typography/color on a
   prominent element; `minor` = small offset or subtle color), and a suggested fix. Attach
   both screenshot paths. A difference is acceptable only if it is a documented, agreed
   deviation (e.g. a feature the design shows that is not built yet and is visibly
   unavailable) — list those separately, never silently. For orchester-app the agreed
   deviations are listed below.
6. The implementer fixes; the verifier repeats steps 2–5 on the new build. Done means the
   verifier reports no `blocking` or `major` findings.

## orchester-app: start, connect, reach the state

**Design frames.** `data/orchester2.pen`, through the `pencil` MCP or, where the main checkout has
it, the read-only wrapper in its `.tmp/kontur-design/` (`./px.sh <pen> <js>`; `Get("<layer>",
{depth: 1})` lists a layer's frames, `Export(["<frame>"], "png", "<dir>", {scale: 1})` exports
one). The layers: Overview `vgqpM`, Tasks `BnbgX`, Task page `iwrwB`, New task `ZLTEI`. A frame is
the whole window at 1440 × 900 (exports are 1442 × 902 with the frame's 1 px outline); a
`… · full page` frame shows the whole scroll height, so compare the window first, then scroll the
app and compare the rest.

**Start an isolated app.** In the implementation's worktree:

```text
powershell -File tools/qa-app.ps1 -Name <task> -Scenario <scenario> [-FakeAgents] [-NoRunner]
```

It builds `orchester-app`, the seed example (`crates/desktop-app/examples/seed`) and
`orchester-runner`, copies them into `.tmp/qa/<task>/bin/` (its own `orchester.db`), seeds a new
repository `.tmp/qa/<task>/repo-<scenario>` with the project Kontur (KON) and registers it there,
and starts the app with `ORCHESTER_APP_MCP_START_WITHOUT_FOCUS=1` and
`ORCHESTER_APP_MCP_WINDOW_CONTROL=on`. It prints the seed's report — `project <id>` (the id
`navigate` takes), the repository, and each task's key, title and state — then `pid <pid>` and
`discovery <path>`, and returns while the app runs. It writes nothing outside `.tmp/qa/` (never
the user's install `E:\projects\orchester` or a real repository), replaces `.tmp/qa/<task>/` on
every start, and refuses to start while programs from `.tmp/qa/<task>/bin/` still run. The app's
console output is in `.tmp/qa/<task>/app.log` and `app.err.log`. `-FakeAgents` builds the
runner's `test-agent` programs, copies `orchester-fake-codex` and `orchester-fake-claude`, and
points the project's Codex connection at the copied fake (any scenario; `live` needs it).
`-NoRunner` copies no runner, so the app cannot start one. Without `-FakeAgents` the profiles
name a Codex connection that is not stored, so nothing can run.

| Scenario | What it holds | Design frames it serves |
|---|---|---|
| `empty` | The catalogue (Explorer, Planner, Builder, Reviewer; Feature delivery v3, Quick fix v2), no task | Tasks and Overview empty project; New task empty |
| `tasks` | The design's 24 tasks (Backlog 3, Todo 8, In progress 8, In review 2, Done 2, Canceled 1; 4 need you), KON-9, 11, 12, 16, 21, 23 and 24 as the design shows them, a held group (KON-17), a blocked task (KON-19). KON-12 runs Implement again after Review returned the work, waits for `cargo add rand` to be approved, and has 3 runs, a checkpoint and uncommitted work | Tasks full page and tabs; Overview full page; KON-12's Summary |
| `task-states` | KON-1 … KON-18, one per state: ready draft, queued, running, paused, token limit, daily budget, interrupted, failed, cancelled, question, approval, plan v1, plan v2, waits after retries, ready for review, merged, unreadable, draft missing details (the seed prints the list) | Summary states |
| `conversation` | `tasks` with a long transcript on KON-12's run: a failed command, a lost stream, a message delivered mid-turn, still working; its approval answered | Conversation |
| `changes` | `tasks` plus KON-25 (checkpoint too large to show), KON-26 (uncommitted work too large), KON-27 (no checkpoint, uncommitted work with a long line); KON-12 has 4 files checkpointed and 2 not, KON-16 none yet, KON-1 its worktree removed | Changes |
| `interrupted` | `tasks` whose application database's last session never ended; KON-12, KON-16 and KON-21 lose their agents | Overview interrupted runs |
| `unreadable-db` | A project database one schema version too new: opening it fails with `SchemaTooNew` | Tasks load error |
| `live` | Six drafts ready to run with the fake Codex (needs `-FakeAgents`): KON-1 ends ready for review, KON-2 asks to approve `cargo add serde`, KON-3 asks a question with options, KON-4 waits for its plan, KON-5 keeps working, KON-6 fails and waits for a person | Running, approval, question and plan states, live |

Runs of the static scenarios are recorded, not executed: a runner that attaches records their
running runs interrupted and ends the approvals they waited for. Capture running states of
static scenarios with `-NoRunner`; reach live states with `live`.

**Connect.** The discovery file `%LOCALAPPDATA%\orchester-app-mcp\<pid>.json`
(`~/Library/Application Support/orchester-app-mcp/<pid>.json` on macOS) holds `pid`, `app`
(`orchester-app`), `url`, `port` and the bearer `token`. Call tools with the client, which sends
the token and keeps it out of its output:

```text
python tools/lurq-mcp-client.py <pid> <tool> --app orchester-app --discovery-dir "%LOCALAPPDATA%\orchester-app-mcp" [--arguments "<json>"] [--image-out <png>]
```

`app_state` must answer `"mcp": {"window_control": true, …}` before you drive the window.

**Reach the state.** Size the window to 1440 × 900 logical with `lurq_resize`, which takes physical
pixels: the logical size times `app_state`'s `window.scale_factor` (`{"width": 2160, "height": 1350}` at
1.5×, `{"width": 1440, "height": 900}` at 1×). Then check that `app_state`'s `window` reports 1440 × 900
before you compare. Then `navigate`, which takes
the route's params and answers the route the tab shows, for example:

- `{"route": "tasks", "project": "<id>"}`, with `"tab"` `all`, `needs_you`, `backlog`, `todo`,
  `in_progress`, `in_review` or `done`, `"q"` (the search) or `"highlight": "KON-12"`;
- `{"route": "overview", "project": "<id>"}`, with `"tab"` `running`, `needs_you`, `queued` or
  `failed`;
- `{"route": "task", "project": "<id>", "key": "KON-12"}` (Summary), with `"tab"` `conversation`
  or `changes`; on changes `"file": "src/webhooks/sender.rs"` with `"from"` `checkpoint` or
  `worktree`;
- `{"route": "new_task", "project": "<id>"}`, `{"route": "edit_task", "project": "<id>", "key": "KON-20"}`.

Then `lurq_read_tree`, `lurq_interact`, `lurq_set_value`, `lurq_wait`, and `lurq_screenshot` with
`--image-out .tmp/verify/<screen>/app.png`.

**Stop it.** Close the app gracefully with its own close button: `lurq_read_tree` lists it as
`#window-close` with a `ref`, and `lurq_interact` `{"ref": "<ref>", "action": "click"}` clicks it; the
app ends what it started and removes its discovery file. If that is not possible,
stop its children first — its runner, whose parent pid is the app's — then the app, and only those
pids:

```text
Get-CimInstance Win32_Process -Filter "ParentProcessId = <pid>" | ForEach-Object { Stop-Process -Id $_.ProcessId }
Stop-Process -Id <pid>
```

The next app's start removes the discovery file a stopped app left.

**Agreed deviations (D4/D6)**, reported in their own list, never as matches: the step rail, the
conversation's search and filters, the Details pane, task selection and filters, the
organization fields (status, priority, tags), the run permissions popover, the 960 px layouts
and the light theme; approvals are answered only with the cards in the conversation (no approval
dialog), and Always allow is shown unavailable with its reason. Data the seed cannot reproduce
is not a mismatch either — run numbers (#128), the keys of `task-states`, clock times, counts
such as runs started — but judge truncation and wrapping with the seeded text. A state no
scenario or tool reaches yet (loading, runner could not restart) is reported as not reachable.

## Rules for the verifier

- Several agents may run Orchester at the same time, and every instance writes its MCP
  discovery file to the same folder. Never pick "the newest" discovery file: take the pid of the
  process you started, open `<discovery folder>\<that pid>.json` (orchester-app:
  `%LOCALAPPDATA%\orchester-app-mcp`; the deprecated desktop: `%LOCALAPPDATA%\lurq\mcp`), and
  before every batch of actions confirm the connection still belongs to that pid (`lurq_windows`
  / the discovery file's `pid`). If it does not, stop and reconnect — never act on another instance.
- The discovery file holds a bearer token. Never print, log, paste or commit it; let
  `tools/lurq-mcp-client.py` read it.
- Run your own copy of the binaries in your own folder when another agent or the user may be
  using the same `target/` directory, so each instance has its own `orchester.db` next to it.
- On Windows a debug build is a console program: its console window belongs to the same pid as
  the app window. Never take "the first window of the pid" as the app. Identify the app window
  through the embedded MCP server (`lurq_windows`), or by winit's window class, and remember that
  the app window exists only after the first frame has been presented; until then the pid may
  have only its console window.

- Do not edit implementation code; report. (Fixing is the implementer's job, so the next
  comparison is independent again.)
- Do not accept "lurq cannot do this" without evidence; name the missing capability so it can
  be added to lurq.
- If a state cannot be reached in the app, say so explicitly — never report it as matching.
- Kill only the processes you started.
