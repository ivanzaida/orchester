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
   `get_screenshot` for the screen node, or `export_nodes` at scale 1 into the run's own storage,
   `$ORCHESTER_RUN_STORAGE/verify/<screen>/design.png` (`.orchester/runs/<run>/`; outside a run, `.tmp/verify/`).
   Never Read or Grep the `.pen` file, and never edit it.
2. **App screenshot in the same state.** For orchester-app, follow "orchester-app: start,
   connect, reach the state" below. For the deprecated desktop, build and run the implementation with
   `cargo run -p orchester-desktop` (every build includes the embedded MCP server; set
   `ORCHESTER_MCP_START_WITHOUT_FOCUS=1` so the window does not take focus). Retain the
   pid of *your* process and connect with `mcp__Orchester__call_run_app(pid, tool, arguments)`. Resize the window to 1440 × 1020 logical
   with `lurq_resize`, which takes physical pixels: the logical size times the display's scale factor
   (2160 × 1530 at 1.5×) (the 32px custom title bar from "Window chrome" in the design doc, the
   1440 × 960 screen frame and the 28px status bar from "Status bar"; composition example
   `cpoal`), drive the app into the designed state
   (`lurq_read_tree`, `lurq_interact`, `lurq_set_value`), wait for idle (`lurq_wait`),
   and call `lurq_screenshot` through the relay; open and cite its returned PNG path. Use fixture data that
   matches the design's content where the app allows it, so text lengths are comparable.
   Use a throwaway git repository under `.tmp/`, never a real project.
3. **Look at both images** at full size, region by region — do not rely on the element tree
   or on the code.
   - Open each PNG to look at it. In an Orchester run, open it with Orchester's `read_file`
     (`mcp__Orchester__read_file`; Claude Code's own `Read` is not available there). Outside
     Orchester, use the `Read` tool.
   - Never use Pencil's built-in browser (`mcp__pencil__browser`: load-page, return-screenshot,
     screenshot-to-canvas) to view a PNG or the app. It is one browser shared by every agent and
     designer connected to Pencil, it takes one capture at a time, and a capture left unfinished
     locks it for everyone ("A capture is already running") until Pencil restarts.
   - If no tool in your step shows an image, say so in the report, and compare from the element
     tree and pixel samples only as a stated fallback.

   Compare:
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

## What you could not check: `unverified`

A state you cannot reach, a design node or app you cannot capture, a fixture that does not exist: say so,
check everything else, and end a pipeline step with the outcome `unverified`, listing each check you could
not do in the result's `unverified` list (`{"check": "Summary 02.B", "reason": "no seed scenario reaches
it"}`). That is no verdict on the work: the run waits for the person, nothing returns to Implement and no
retry is used. The person accepts the step as it is (only a person, or automation they let answer approvals),
runs it again, or returns the work with a note. Use
`failed` only for mismatches you found (`blocking` or `major`), even when other checks were unverified, and
`question` only for something the person can do now that lets you go on in this turn.

Orchester does not run a Verify again on the revision its last failed or unverified result was for: with
nothing changed (no new commit, and so no new fixture either, since the seed lives in the repository), the
run waits for the person instead, unless they ask for the step again.

## orchester-app: start, connect, reach the state

**Design frames.** `data/orchester2.pen`, through the `pencil` MCP or, where the main checkout has
it, the read-only wrapper in its `.tmp/kontur-design/` (`./px.sh <pen> <js>`; `Get("<layer>",
{depth: 1})` lists a layer's frames, `Export(["<frame>"], "png", "<dir>", {scale: 1})` exports
one). The layers: Overview `vgqpM`, Tasks `BnbgX`, Task page `iwrwB`, New task `ZLTEI`. A frame is
the whole window at 1440 × 900 (exports are 1442 × 902 with the frame's 1 px outline); a
`… · full page` frame shows the whole scroll height, so compare the window first, then scroll the
app and compare the rest.

**Start an isolated app.** As a step of an Orchester run, call
`mcp__Orchester__start_qa_app` (Codex: `start_qa_app` of the Orchester server) with
`{"scenario": "<scenario>", "fake_agents": null, "no_runner": null}`. Orchester runs the worktree's
`tools/qa-app.ps1` for you on the scenario, in `.tmp/qa/run-<number>-<step>/`, and answers the copy's pid.
As many QA apps run on the computer at once as the project's *QA apps at once* allows (Project settings ›
Runs › Limits, default 2): while other runs' QA apps hold every QA slot, the call waits and the run's transcript
says *Waiting for a QA slot*; never write lease, ACK or hold notes, and never
start the script yourself. Calling it again replaces your QA app; Orchester stops it, and frees the slot,
when your turn ends, and also while your turn waits for a person (a tool approval, a question): start it
again once you go on. A person is asked to approve the start (*Approve similar* covers later starts only
while `tools/qa-app.ps1` is unchanged), since it builds and runs the worktree's code outside the sandbox;
the request names the build scripts changed since the run's base. Only a step that may write the worktree
and has network access is offered `start_qa_app`, so give the Verify step those.

Outside a run (an agent session in a checkout of its own), start the copy in the implementation's worktree
with the script:

```text
powershell -File tools/qa-app.ps1 -Name <task> -Scenario <scenario> [-FakeAgents] [-NoRunner]
```

It builds `orchester-app`, the seed example (`crates/desktop-app/examples/seed`) and
`orchester-runner`, copies them into `.tmp/qa/<task>/bin/` (its own `orchester.db`), seeds a new
repository `.tmp/qa/<task>/repo-<scenario>` with the project Kontur (KON) and registers it there,
and starts the app with `ORCHESTER_APP_MCP_START_WITHOUT_FOCUS=1`,
`ORCHESTER_APP_MCP_WINDOW_CONTROL=on` and `ORCHESTER_APP_MCP_DISCOVERY=.tmp/qa/<task>/mcp/` (a free
port and a new token on each start, in `<pid>.json` there; never the installed app's fixed address).
It prints the seed's report — `project <id>` (the id `navigate` takes), the repository, and each
task's key, title and state — then `pid <pid>`, `mcp <folder>` and `discovery <path>`, and returns
while the app runs. It writes nothing outside `.tmp/qa/` (never
the user's install `E:\Orchester`, the former one `E:\projects\orchester`, or a real repository), replaces `.tmp/qa/<task>/` on
every start, and refuses to start while programs from `.tmp/qa/<task>/bin/` still run. The app's
console output is in `.tmp/qa/<task>/app.log` and `app.err.log`. `-FakeAgents` builds the
runner's `test-agent` programs, copies `orchester-fake-codex` and `orchester-fake-claude`, and
points the project's Codex connection at the copied fake (any scenario; `live` needs it).
`-NoRunner` copies no runner, so the app cannot start one. Without `-FakeAgents` the profiles
name a Codex connection that is not stored, so nothing can run.

To pick a scenario, list them with `cargo run -p orchester-app --example seed -- --list-scenarios` (one per line,
sorted). The seed checks the name: an unknown one exits non-zero with the known ones, writes nothing, and
`tools/qa-app.ps1` stops before starting the app. A new scenario is added only in the seed's registry
(`ALL` in `crates/desktop-app/examples/seed/kontur/scenarios/mod.rs`), never in `tools/qa-app.ps1`: the script
names no scenarios, so its git blob, which the project rule *Approved for every run* for `start_qa_app` is kept to,
stays the same. The table below describes some of them.
| Scenario | What it holds | Design frames it serves |
|---|---|---|
| `empty` | The catalogue (Explorer, Planner, Builder, Reviewer; Feature delivery v3, Quick fix v2), no task | Tasks and Overview empty project; New task empty |
| `tasks` | The design's 24 tasks (Backlog 3, Todo 8, In progress 8, In review 2, Done 2, Canceled 1; 4 need you), KON-9, 11, 12, 16, 21, 23 and 24 as the design shows them, a held group (KON-17), a blocked task (KON-19). KON-12 runs Implement again after Review returned the work, waits for `cargo add rand` to be approved, and has 3 runs, a checkpoint and uncommitted work | Tasks full page and tabs; Overview full page; KON-12's Summary |
| `task-states` | KON-1 … KON-18, one per state: ready draft, queued, running, paused, token limit, daily budget, interrupted, failed, cancelled, question, approval, plan v1, plan v2, waits after retries, ready for review, merged, unreadable, draft missing details (the seed prints the list) | Summary states |
| `conversation` | `tasks` with a long transcript on KON-12's run: a failed command, a lost stream, a message delivered mid-turn, still working; its approval answered | Conversation |
| `changes` | `tasks` plus KON-25 (checkpoint too large to show), KON-26 (uncommitted work too large), KON-27 (no checkpoint, uncommitted work with a long line); KON-12 has 4 files checkpointed and 2 not, KON-16 none yet, KON-1 its worktree removed | Changes |
| `interrupted` | `tasks` whose application database's last session never ended; KON-12, KON-16 and KON-21 lose their agents | Overview interrupted runs |
| `unreadable-db` | A project database one schema version too new: opening it fails with `SchemaTooNew` | The project that cannot be opened |
| `tasks-unreadable` | The project opens; its one task's stored status is unknown, so reading the task list fails with `Corrupt` | Tasks load error |
| `decisions` | Runs ready for review (one conflicts with the moved `main`, one merges cleanly, one has no branch), one that waits after Review's retries, and *Check the empty states on screen*, whose Review could not check two states and waits unverified | Ready for review, merge conflict; waits after retries (`DVuXu`); waits unverified (no frame of its own: compare with `DVuXu`'s parts) |
| `live` | Six drafts ready to run with the fake Codex (needs `-FakeAgents`): KON-1 ends ready for review, KON-2 asks to approve `cargo add serde`, KON-3 asks a question with options, KON-4 waits for its plan, KON-5 keeps working, KON-6 fails and waits for a person | Running, approval, question and plan states, live |
| `orchestrator` | The Orchestrator on *Claude · Work* (Opus 5.5, high). Sign-ins for the runs-on picker: Claude Code *Work* (Max, 42 % of 5 h used), *Personal* (Pro, 5-hour limit reached for 80 more minutes), Codex *Client* (signed out), each with a connection; DeepSeek API providers. *Plan the webhook retry work* (opens first): a digest *4 waiting for you* of waits that stand (KON-5's question, KON-6 … KON-8 interrupted), then a turn with actions. *Every card* (History): a run's end that stands (KON-1, ready to merge), waits *Answered by the Orchestrator*, *Answered by you*, *Waits for you* (KON-2's plan) and *Orchestrator is reading* (KON-3's question), waiting decision cards (start KON-4, merge KON-1, cancel KON-2, publish Builder v2 with network on, add the MCP server `figma`: 6 facts, 3 shown and *Show details*; an answer, a release), folded decisions, a hold, a daily report, a stopped and a failed turn | Orchestrator · redesign 01, 03, 04, 05, 05b; Orchestrator cards · redesign and 06 |
| `orchestrator-not-set-up` | `orchestrator` with nothing chosen for the Orchestrator | Orchestrator · redesign 02 |
| `project-settings` | `connections`, plus `acme/kontur` linked and expecting `dana-k` while `gh` is signed in as `ivanzaida`; base branch `release/2.4`, which is gone; secrets in Development, Staging and Production (`DEEPSEEK_API_KEY` used by the tool `deploy-preview`, `FIGMA_TOKEN` rotated and used by the MCP server `figma`, `GITHUB_APP_KEY` unused, `OLD_FIGMA_TOKEN` archived), records only; `figma` on with `FIGMA_API_KEY` from `FIGMA_TOKEN`, two tools run without asking and the profile Design reviewer using it | Project settings `XGtkg`: 01b, 05, 06, 06b (open `FIGMA_TOKEN`), 13 (open `figma`), 14 (open GitHub) |
| `project-settings-shared` | `project-settings` linked as owner to `orchester.example.com/kontur` with four members as last read; no server answers and this computer's server list lacks it, so the address says so and *Leave* is unavailable with why | 04b Sharing · shared (its address hint and *Leave* differ from the frame, which draws a connected server) |
| `settings` | App Settings with a person's data, fixture-only (`tools/qa-app.ps1` says how). Accounts: Claude Code *Work* (signed in, plan readings, a running run on it: Remove is blocked), *Personal* (unused), *Old* (Sign-in expired), Default (Not signed in); Codex *Client* (signed in, plan readings, a connection on it), *Archive* (Folder missing), Default; stand-in CLIs answer the checks the same way. Notifications: quiet hours 22:30–07:00, daily report at 18:30. Automation: the installed app's MCP rows (address, port, token, Connect an agent) on a free port with a token in memory, *claude-code* and *codex-mcp-client* connected. Orchester Cloud: an invite to Billing waiting; kept here, Mobile team's join waiting for approval and Docs team's that didn't finish; team servers *Kontur team* (Connected, Kontur linked), *Build box* (Offline about two minutes after the start; Reconnecting before), *Old office* (Certificate changed), *Design team* (signed out here, its own certificate) and *Team box* (sign-in ended) | `App settings · redesign` `iUmo3` in the new `orchester.pen`: 01–05c and the states sheets `poD7g`, `shpsk`, `ZUuMT` |

Runs of the static scenarios are recorded, not executed: a runner that attaches records their
running runs interrupted and ends the approvals they waited for. Capture running states of
static scenarios with `-NoRunner`; reach live states with `live`.

**Connect.** Use the per-invocation relay tool `mcp__Orchester__call_run_app(pid, tool, arguments)`
(Codex: `call_run_app` of the Orchester server). Leave out `pid` to call the QA app `start_qa_app` started
in this turn: the relay reads that app's own discovery file and searches nothing. For an app you started
with the script, pass the exact pid it printed.
It runs without asking only for an app whose resolved executable is inside this run's worktree,
using the same process boundary as `stop_run_process`. The relay finds the exact-pid discovery file
in the user's discovery folder or a QA copy's folder `.tmp/qa/<task>/` in the worktree, checks its
pid and app name, and keeps the token private; a QA copy's file is the `.tmp/qa/<task>/mcp/<pid>.json`
that `tools/qa-app.ps1` writes (or `.tmp/qa/<task>/profile/<folder>/<pid>.json` for a QA profile). It
looks nowhere else under `.tmp/`. Use it instead of running `tools/lurq-mcp-client.py`,
reading discovery or escalating a script outside the sandbox.

For example, call `call_run_app` with `{"tool":"app_state","arguments":{}}` for the QA app, or
`{"pid":12345,"tool":"app_state","arguments":{}}` for one the script started.
Every embedded tool uses this same relay, including `navigate`, `lurq_resize` and `lurq_screenshot`.
PNG images are saved in the run's own storage, `.orchester/runs/<run>/screenshots/` (the folder
`ORCHESTER_RUN_STORAGE` names), which the task page lists under Artifacts and cleaning or reusing a workspace never
removes; the answer gives its absolute path to open with `read_file` (step 3) and cite.

`app_state` must answer `"mcp": {"window_control": true, …}` before you drive the window.

**Reach the state.** Size the window to 1440 × 900 logical with `lurq_resize`, which takes physical
pixels: the logical size times `app_state`'s `window.scale_factor` (`{"width": 2160, "height": 1350}` at
1.5×, `{"width": 1440, "height": 900}` at 1×). Then check that `app_state`'s `window` reports 1440 × 900
before you compare. Then `navigate`, which takes
the route's params and answers the route the tab shows, for example:

- `{"route": "tasks", "project": "<id>"}`, with `"tab"` `active` (the default), `backlog`, `done` or `all` (the
  list's views since ORC-99; the old workflow tab names are refused with `unknown_tab`), `"q"` (the search) or
  `"highlight": "KON-12"`;
- `{"route": "overview", "project": "<id>"}`, with `"tab"` `running`, `needs_you`, `queued` or
  `failed`;
- `{"route": "task", "project": "<id>", "key": "KON-12"}` (Summary), with `"tab"` `conversation`
  or `changes`; on changes `"file": "src/webhooks/sender.rs"` with `"from"` `checkpoint` or
  `worktree`; `"view": "memory"` opens the task's memory (from Summary's properties) and
  `"view": "local-integration"` Local integration (Changes' *Try locally*). The former tabs
  `details`, `artifacts`, `memory` and `local-integration` are refused with `unknown_tab`;
- `{"route": "new_task", "project": "<id>"}`, `{"route": "edit_task", "project": "<id>", "key": "KON-20"}`;
- `{"route": "project_settings", "project": "<id>"}` (General), with `"tab"` `runs`, `context`, `connections`,
  `secrets` or `sharing`, and with tab `runs` `"view": "rules"` (*Approved for every run*). Connections and Secrets
  keep their own routes, which open those tabs: `connections` with `"selected"` (a connection's id) opens its page,
  `new_connection` with `"kind"` `agent_provider` or `mcp_server` the add pages, `edit_connection` with
  `"connection"` an MCP server's form;
- `{"route": "settings", "tab": "<tab>"}`, `tab` `automation` (the default), `general`, `notifications`,
  `accounts`, `orchester` (Orchester Cloud), `connect` or `invite`, and what the tab opens on, without a click:
  - accounts: `"account"` (an id, a name such as `"Work"`, or `claude-default` / `codex-default`) for its page
    (03b), with `"panel"` `rename` or `remove` for the question in place, `sign_in` for step 2; `"panel": "add"`
    alone for *Add account* (03c);
  - orchester: `"server"` (an id, a name such as `"Old office"`, or a host) for its page (05c); `"ask":
    "sign-out"` for *Sign out of Orchester Cloud?*, or with `server` that server's *Sign out of …?*; `"ask":
    "decline", "invite": "<invite id>"` (the id in the invite row's element ids) for *Decline the invite?*;
  - connect: `"server"` and `"step"` `address`, `compare`, `join` or `username` (05b, 05b2): past the address it
    asks what answers there; a server with its own certificate stays at *Compare the check code* (the
    `settings` scenario's *Design team*), since only a person answers it;
  - invite: `"server"` and `"project"` (its slug, e.g. `billing` on *Kontur team*) for the invite landing;
  - automation: `"ask": "rotate-token"` for *Rotate the token?*.

  Opening a page or a question is navigation and works while Settings refuses window input; answering it stays a
  person's, and a synthetic press is refused.

Then call `lurq_read_tree`, `lurq_interact`, `lurq_set_value`, `lurq_wait`, and `lurq_screenshot`
through `call_run_app`. Open the returned PNG path and include it in the verification report.

**Stop it.** A QA app `start_qa_app` started is stopped by Orchester when your turn ends; stop it
earlier only to free its QA slot for another run. Otherwise close the app gracefully with its own close button: `lurq_read_tree` lists it as
`#window-close` with a `ref`, and `lurq_interact` `{"ref": "<ref>", "action": "click"}` clicks it; the
app ends what it started and removes its discovery file. If that is not possible,
use `mcp__Orchester__stop_run_process(pid)` to stop the app and its children inside the run's
worktree. Stop only the app you started; never run `Stop-Process` or `kill` for this.

The next app's start removes the discovery file a stopped app left.

**Agreed deviations (D4/D6)**, reported in their own list, never as matches: the step rail, the
conversation's search and filters, the Details pane, task selection and filters, the
organization fields (status, priority, tags), the run permissions popover, the 960 px layouts
and the light theme; approvals are answered only with the cards in the conversation (no approval
dialog), and Always allow is shown unavailable with its reason. Data the seed cannot reproduce
is not a mismatch either — run numbers (#128), the keys of `task-states`, clock times, counts
such as runs started — but judge truncation and wrapping with the seeded text. A state no
scenario or tool reaches yet (loading, runner could not restart) is reported as not reachable,
and listed in the step's `unverified` result.

## Rules for the verifier

- Several agents may run Orchester at the same time. Use only the app you started: the QA app of
  `start_qa_app` (no pid) or the pid you started, with `mcp__Orchester__call_run_app`; never select the
  newest discovery file. The QA slots are Orchester's to hand out: wait in `start_qa_app`, never in notes. Before each batch,
  call `lurq_windows` or `app_state` through the relay and confirm the intended app state.
- The relay owns discovery credentials. Never read, print, log, paste or commit a discovery
  token, and never pass one as a tool argument.
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
- If a state cannot be reached in the app, say so explicitly — never report it as matching — and
  end with `unverified` listing it, not with `failed`.
- Kill only the processes you started.
