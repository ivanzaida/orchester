# Orchester — agent testing: services, stacks, test suites and UI drivers

Date: 2026-10-28. Status: **agreed direction** (the person, in chat, 2026-10-28). Design: the desktop file
`crates/desktop-app/design/orchester.pen`, board
`Agent testing · redesign` (`eWydp`).

## Why

Agents start dev servers by shell, forget to stop them, read megabytes of test output, rerun everything in Review, and
ask a person to kill processes Orchester could not read (OP-33 waited on `taskkill //PID …` for that). Runs loop and
stall. Testing becomes part of Orchester instead: Orchester starts the project's apps for a run, watches them, runs the
project's test suites and hands agents structured results.

It works the same for **web apps** and **native apps** (Windows and macOS), and for repositories with several apps
that depend on each other (Orchester Platform: cloud server, cloud web, team server, team web).

## Decisions (the person, 2026-10-28)

1. Everything is configured **in Orchester**: services, stacks and test suites live in the project database with drafts
   and published versions, like tools and pipelines, and travel with a shared project's configuration through the team
   server. Nothing is added to the repository; readiness and seed data are defined in Orchester, so an app needs no
   health endpoint or seed script of its own.
2. One set of agent tools for web and native apps; only the UI driver underneath differs.
3. **Services at once** is a limit of the computer, set in the app's settings; each project has its own share of it, as
   agents working at once do today.
4. A quarantined flaky test still runs, but its failure never fails the step; it is shown so the person sees when it is
   stable again.
5. New Orchester runs for this work use the Claude 2 pipelines.
6. **First only programs Orchester starts itself** (the person, 2026-10-28): services, test suites, and the `native`
   and `mcp` drivers. Web pages are automated later through Orchester's own in-app Browser, not a separate headless
   browser; the `browser` driver does so (ORC-256, built below), the `cdp` driver is not built (ORC-260 deferred).

## Concepts

### Service

A program Orchester starts for a run.

| Field | Meaning |
| --- | --- |
| program, arguments | No shell. `{{port}}`, `{{url}}`, `{{run}}`, `{{data}}` and `{{services.<name>.url|port}}` are filled per run |
| working folder | Inside the run's worktree |
| variables | Plain values, or a project secret by name (never shown, never logged) |
| needs | Services started first, in order; each must be ready before the next |
| readiness | The port accepts connections; an HTTP 2xx/3xx on a path; or a log line matching a pattern. Plus a timeout; on timeout the start fails with the last log lines |
| seed | Runs once after ready: a sequence of HTTP requests whose answers may be captured (`team_id ← $.id`) for later requests and for agents, or a command |
| per-run data | None; a fresh database container; or files in the run's own folder. Deleted when the run ends ([per-run data](#per-run-data)) |
| UI driver | `none`, `browser`, `native`, `cdp` or `mcp` (see [UI drivers](#ui-drivers)), and where `ui_open` starts |
| owned paths | Repository paths (`web/cloud/**`); a run that changed them starts this service in its Test step |
| approval | The first start asks a person; *Approve for every run* keeps answering while the program, arguments, variables and seed are unchanged |

Ports and data are per run, so parallel runs never clash. A service lives for the step that started it: Orchester stops
it when the step ends, when the run pauses or fails, and while the run waits for a person (as QA apps today), and
releases its slot. A crash is reported to the agent as an event with the exit code and the last log lines, and to the
person in the Conversation.

Built (ORC-274): a start that fails names the first service that failed (the root cause), its exit code or its
readiness timeout and its last 20 lines, and the services not started because of it. A ready service whose program
exits on its own crashed: the services the step runs that need it, also through others, are stopped; the answer of
the agent's next Orchester tool call carries a notice after it, as a text block of its own (service, exit code, how
long it ran, the services stopped, its last 20 lines); and the run's Conversation records a `service_crashed` runner
note with the same facts (`orchester_core::service::ServiceCrash`). The crash card's look is ORC-265.

#### Per-run data

Built (ORC-251; the person chose the shape, 2026-11-05):

- **Run folder.** `.orchester/runs/<run>/data/<folder>` in the run's storage, made before the service starts and
  passed as `{{data}}` (only a service whose per-run data is a run folder may use it; publishing checks). It lives
  through the run's steps and goes with the whole `data/` folder when the run ends — succeeded, failed or cancelled —
  tried again for a few seconds while a stopping service still holds a file.
- **Database container.** The service *is* the container: its program is the image (`postgres:16-alpine`), its
  arguments go to the container's command, its plain variables are set by value and its secrets by name only (the
  value reaches `docker` in its environment). The image's lowest exposed TCP port is published on `127.0.0.1:{{port}}`,
  so `{{port}}` and `{{services.<name>.port}}` reach it; an image the computer lacks is pulled first (up to 10 min).
  It runs as `docker run --rm` named `orchester-<run>-<service>-<port>` with the labels `orchester.project`,
  `orchester.run` and `orchester.service`, and is removed when the service stops (its step ends, the run pauses, waits
  for a person or is cancelled). The run's storage notes each container (`containers/<name>`); when the run ends the
  containers still noted are removed, and a runner that starts removes the noted containers that carry its project's label
  — what a runner that died left — and the data folders of runs that ended. Readiness by an open port says ready as
  soon as Docker publishes it, before the database listens: a log line is the better check.
- **No Docker.** `docker` is looked up in the project's `PATH`, else the runner's, as a service's program is. When it is
  missing, or its engine does not answer `docker version` within 15 s, the start fails at once with *Docker is not
  available*, naming the folders searched or the program asked and its answer (`orchester_exec::docker::
  DockerUnavailable`). On macOS any Docker whose CLI is on that `PATH` works: Docker Desktop, OrbStack or colima.

### Stack

A named set of services (`cloud`, `team`, `full`). Starting a stack resolves `needs` into start steps; services on one
step start together. A stack takes as many **Services at once** slots as it has services; while all are taken, a start
waits and says so.

### Test suite

Program and arguments with `{{filter}}`, a report format (cargo json, JUnit XML, vitest json, Playwright json, TAP),
a layer (lint, unit, e2e), the services or stack it needs (started before, stopped after), artifact paths, a timeout
and a filter. Orchester reads the report into one result shape: per test its name, outcome, duration, message, file
and line, and artifacts.

## Agent tools

| Tool | Does |
| --- | --- |
| `list_services`, `list_test_suites` | What the project defines, with needs and drivers |
| `start_service(name)` | Starts it and its needs in order, waits for ready, runs the seed; returns URL, port and captured seed values |
| `start_stack(name)` | Starts a stack's services and their needs in order, each once; returns each one's URL and port |
| `service_status(name)`, `service_logs(name, tail, since)`, `stop_service(name)` | State, log tail (secrets redacted), stop without asking |
| `run_tests(suite, filter?)` | Runs a suite (starting what it needs) and returns the structured result: counts, each failure with message, file:line, a short excerpt and its artifacts |
| `test_output(run, test)` | One test's whole output, when it is really needed |
| `ui_open(service)`, `ui_snapshot()`, `ui_act(ref, action)`, `ui_screenshot()`, `ui_logs()`, `ui_wait(condition)` | See and use the service's UI through its driver |

Agents are told never to start servers or test runners by shell. `start_qa_app` and `call_run_app` become the
`desktop-qa` service with the `mcp` driver.

## UI drivers

| Driver | Snapshot and act | Screenshot | Logs |
| --- | --- | --- | --- |
| `browser` | A tab of the run's own in Orchester's in-app Browser (the project's headless browser, over CDP): accessibility tree with refs, actions by ref; it may open only the run's own service origins | The page | Console errors, failed requests and what was blocked |
| `cdp` | Electron and WebView2 apps through their remote debugging port | The window's web content | Console |
| `native` | Windows UI Automation; macOS Accessibility (AX). Actions through element patterns (invoke, value, toggle, scroll), never global input | Only that window (Windows: the window's capture; macOS: ScreenCaptureKit by window id) | stdout/stderr |
| `mcp` | Apps with their own MCP (lurq, Orchester): the existing relay | Through the app | Through the app |

Rules for every driver: only processes the run started (as `stop_run_process` checks), never global input, never a
whole-screen capture (AGENTS.md rule 16), windows start off-screen and without focus where the OS allows it. An
element without the needed pattern answers an explicit *unsupported for this element*. On macOS the native driver
needs Accessibility and Screen Recording; without them it shows a visible state naming the missing permission and where
to grant it. Platform code lives in small `cfg(target_os)` modules; FFI with `unsafe` in dedicated modules (rule 9).

Built (ORC-738): the `ui_*` interface and the `mcp` driver (`orchester_core::orchester_tools::ui`, the runner's
`execution::step::ui`). Every driver answers in one shape: a snapshot is a tree of elements, each with a ref, a role
and a name; `ui_act` takes `click`, `type`, `key`, `scroll` or `select`; a screenshot is a PNG saved in the run's
storage, answered as a path `read_file` shows. The `mcp` driver maps each call onto one of the app's lurq tools
(`lurq_windows`, `lurq_inspect`, `lurq_act`, `lurq_interact`, `lurq_set_value`, `lurq_screenshot`, `lurq_logs`,
`lurq_wait`) over the run's relay to the app; `ui_wait` for an element inspects until it is shown. The QA app is the
built-in service `desktop-qa` with the `mcp` driver, offered to every step that may start a QA app: `start_service`
starts it on the `tasks` scenario with the QA script's approval, `start_qa_app(scenario)` stays its alias and
`call_run_app` the raw relay. Until *Services at once* exists, a QA app's service slot is one of the computer's QA
slots, as many as the project's *QA apps at once*. Only `desktop-qa` is driven yet: a project's service with the `mcp`
driver answers that the driver reaches only the built-in one.

Built (ORC-257): the `native` driver on Windows (`orchester_exec::native_ui`, the runner's `execution::step::ui::native`).
A published service with it brings the `ui_*` tools (`StepTools::with_native_ui`). Each call finds the service's
windows again: the visible top-level windows of its program's tree whose processes pass `stop_run_process`'s check
(started since the run began, executable inside the worktree, not the runner or the agent CLI —
`RunProcesses::tree`); a tree with none answers *refused* with each process's reason. `ui_open` moves them beyond the
desktop without activating them (`SetWindowPos`, `SWP_NOACTIVATE`) and says whether any monitor still shows them. A
snapshot is each window's UI Automation control view, at most 300 elements 40 deep: a ref minted from the element's
runtime id (`u42.…`, stable while the element lives), its control type as the role (`button`, `edit`, `checkbox`, …),
its name, its states (`disabled`, `focused`, `checked`/`unchecked`/`mixed`, `expanded`/`collapsed`, `selected`,
`readonly`, `password`) and its value (never a password field's). `ui_act` uses patterns only: `click` Invoke, else
Toggle, SelectionItem or ExpandCollapse; `type` Value; `scroll` ScrollItem; `select` the SelectionItem of an option
inside it (opening it first when it is collapsed), else Value; an element without one answers *unsupported for this
element*, and `key` always answers that the driver presses no keys. `ui_screenshot` is the window's own rendering into
memory (`PrintWindow` with `PW_RENDERFULLCONTENT`), or that cropped to an element; `ui_wait("idle")` waits until each
window's thread handled a `WM_NULL`; `ui_logs` reads the service's own output. The tests drive
`orchester-test-window`, a Win32 fixture in the runner's test support (`tests/native_ui.rs`).

Built (ORC-258): the `native` driver on macOS (`orchester_exec::native_ui::macos`; its platform-independent mapping in
`native_ui::accessibility` and `native_ui::permission`). Every `ui_*` call of such a service first asks
`native_ui::ready()`: without Accessibility (`AXIsProcessTrusted`) or Screen Recording (`CGPreflightScreenCaptureAccess`)
it answers *permission missing*, naming each missing one and its pane (System Settings › Privacy & Security ›
Accessibility / Screen Recording); nothing prompts. macOS grants them to the responsible app, Orchester, which started
the runner. The workspace exposes the state for the UI as `agent_testing::native_driver::NativeDriverState` (ready,
needs these permissions, not supported), each permission with its System Settings link. The windows are the
normal-layer windows of the reachable processes as the window server lists them (`CGWindowListCopyWindowInfo`), each
matched to its AX window by its window id (`_AXUIElementGetWindow`); `ui_open` moves them beyond the right of the
displays through `AXPosition`, which activates nothing, and says whether one still shows. A snapshot is the
AXUIElement tree with the same bounds: a ref from the element's `CFHash` (`x…`, stable while it lives), its AX role in
the Windows driver's words (`AXButton` is `button`, `AXTextField` `edit`, others lower case without `AX`), its name
(title, else description, else a label's text), its states (`disabled`, `focused`, `checked`/`unchecked`/`mixed`,
`expanded`/`collapsed`, `selected`, `readonly`, `password`) and its value (never a password field's). `ui_act` uses AX
only: `click` `AXPress`, else `AXPick`; `type` sets a settable `AXValue`; `scroll` `AXScrollToVisible`; `select`
presses or picks the option named so inside it (showing a pop-up's menu first), else sets `AXValue`, else steps a
number with `AXIncrement`/`AXDecrement`; an element without one answers *unsupported for this element*, and `key` that
the driver presses no keys. No CGEvent is ever posted. `ui_screenshot` is that window alone by its id through
ScreenCaptureKit (`SCContentFilter` of the desktop-independent window, `SCScreenshotManager`, macOS 14), before macOS 14
the window server's image of that one window (`CGWindowListCreateImage` with `kCGWindowListOptionIncludingWindow`);
`ui_wait("idle")` waits until the app answers an AX read within the timeout. Nothing of it has run on a Mac yet: it is
checked with `cargo check` and `cargo clippy --target aarch64-apple-darwin`, and its pure parts by `orchester-exec`'s
`tests/native_ui.rs`.

Built (ORC-256): the `browser` driver (`UiDriver::Browser`, the runner's `execution::step::ui::browser`), through
Orchester's own in-app Browser, not a browser of its own: a web service opens in a tab of the run's own in the
project's headless browser the runner already drives over CDP (`docs/design/browser.md`), which the person may watch
in the Browser window. So no dependency is added (AGENTS.md rule 17): the runner's own CDP client over
`tokio-tungstenite` (`orchester_runner::browser::cdp`) is reused, and the browser is found as Settings › Browser
finds it (`orchester_exec::browser::find`, or the program the person chose). A published service with it brings the
`ui_*` tools (`StepTools::with_service_ui`). `ui_open` opens one tab per service, blank first, puts the origin guard
in place (CDP `Fetch`: each request paused and let through only to an origin of a service the step runs, or for the
page's own `data:`, `blob:` and `about:` content; any other host or scheme fails as blocked by the client —
`orchester_core::orchester_tools::ui::origins`), then loads where the service's `ui_start` says, else its URL; the
answer names the tab, its address and title, the allowed origins and what was blocked meanwhile. The allowed origins
are those of the services running at each call. The guard covers what the page starts too: the tab auto-attaches to
its workers, service workers and frames of other processes, each held at its start until the same guard is on it
(a dedicated worker's requests are its page's). It cannot attach to a new window a page opens, so none opens: each
document of the tab is answered whole (`Fetch.fulfillRequest`, as the browser reads again only the headers of an
answer given that way) with a `Content-Security-Policy: sandbox` that allows all but popups, and the browser's
refusal of `window.open` or `target=_blank` is on the page's console, which `ui_logs` reads. A snapshot is the
page's accessibility tree, at most 300 elements 40
deep, each ref the element's DOM node (`b` and its `backendNodeId`, stable while the node lives) with its role, name,
states and value; `ui_act` clicks the element's centre, types (replacing a field's value), presses a key in it or the
focused element (`Ctrl+A` style), scrolls it into view, or selects a `<select>` option by label, text or value (else
sets a field's value), all through the tab's own input; a ref whose node is gone says so. `ui_wait` takes `idle`
(loaded, two frames drawn), `element`, and for a web page `selector`, `text` and `url`; the other drivers refuse those
three. `ui_screenshot` is the page or one element, re-encoded into the run's storage. `ui_logs` reads the page's
console errors, its requests that failed or were answered 4xx/5xx, and the blocked requests (at most 200 kept), each
blocked request said once; every answer also carries what was blocked since the last one. A missing browser answers
`browser_unavailable` with what was looked for and what to install. The tabs close when the step's services end
(`StepServices::on_end`), and with the run. Not built: the service editor's *browser* card (`cKQAD`; the driver is set
over MCP, `save_service_draft`), the person's hold on a tab the agent drives (the browser tools' decision 16), and
WebSocket connections, which `Fetch` does not pause. Tests: `cargo test -p orchester-runner --features test-agent
--test browser_ui browser_ui` (a fixture page of the stand-in web server) and `cargo test -p orchester-core --test
orchester_tools ui_browser_origins`.

## The Test step

A pipeline step kind that runs suites in layers — lint, then unit, then e2e — starting what each needs and stopping at
the first failing layer. Only the services and suites whose owned paths the run changed run, plus suites marked always.
A failed test runs once more: passing then marks it **flaky** and does not fail the step. The step's result is stored
with the run; when it fails, the run returns to the writing step with **only the failed tests** (name, message,
file:line, excerpt, artifacts), not the raw output. Review and Verify get the stored result and may run single tests
with `run_tests(filter)` instead of re-running everything.

## Flaky tests and quarantine

A test that is flaky again and again is listed as *watched*; the person moves it to *quarantine* (or back). A
quarantined test still runs; its failure is shown but never fails the step. The list lives in Project settings ›
Services › Flaky tests.

Built (ORC-259): the project database keeps every flaky outcome a Test step sees (suite, test, run, time) and every
failure of a quarantined test. A test flaky in **3 runs within 14 days** is watched — one flaky outcome is often
chance and two can be one bad hour, a third in another run is a pattern; runs are counted, not outcomes, and a fixed
test leaves the list after two quiet weeks (`orchester_core::test_step::flaky`). A quarantined failure is in the
step's result for the steps after it, never in what returns to the writing step. Quarantining goes through
`quarantine_test` (person_only unless *Let automation change agent permissions*) and the workspace's
`agent_testing::flaky`; `release_test` is always taken. The quarantine list is this computer's project data: it does
not travel with a shared project's configuration yet.

Built (ORC-267): Project settings › Services › *Flaky tests* (`Tv9VC`) lists the quarantined tests, then every flaky
test not quarantined (watched first) under *Watching*, each with the runs it was flaky in within the window, when it
was last seen and its suite, and *Unquarantine* or *Quarantine now*; under them the rule, its figures in locked fields
since Orchester fixes them. The history keeps no file and line and no task of the run, so a row says less than the
design's. A *Quarantine now* from an MCP client driving the window is taken only under *Let automation change agent
permissions*, as `quarantine_test` is. The seed's `flaky-tests` scenario holds such a history.

## Import

A first draft of services and stacks can be read once from `docker compose` files, `package.json` scripts and Cargo
binaries; the person keeps or skips each and edits the drafts. The Orchestrator chat may propose services as cards the
person applies (as MCP servers today), since a service runs a program.

Built (ORC-254): the workspace's `agent_testing::import` reads the checkout and runs nothing of it — no git, cargo or
package manager; folders are listed up to 6 deep without following links, past hidden folders, `node_modules`,
`target`, `vendor`, `dist`, `build`, `out` and test folders, and each file is opened through the file tools' scope, at
most 512 KiB (a YAML file whose aliases expand past 100 000 nodes is refused). A compose service with an `image` is a
service whose program is that image, run as a fresh container (`RunData::DatabaseContainer`): its `command` the
arguments (a text a shell would read is reported), its `environment` values the variables (one Compose takes from where
it runs is reported, for a value or a secret), its `depends_on` of the same file the needs, and a `healthcheck` that
asks a local URL an HTTP readiness check of that path (any other test, and its timings, are reported; the port is
waited for); more than one container port is reported. A service Compose builds without an image is passed over with
why. Each compose file's services are a stack named after its `name`, else its folder. A `package.json` script named
`dev`, `start`, `preview` or `serve` (or `dev:…`) is `<manager> run <script>` in the package's folder, the manager the
nearest folder up names by its lockfile or `packageManager` (else npm), so a workspace's packages run through the
workspace's manager; ready on a line showing a local URL. A Cargo binary target (`[[bin]]`, else `src/main.rs` and
`src/bin/`) is `cargo run --bin <name>` with the features it requires, ready when its port opens. Both get `PORT` set to
`{{port}}` and own their folder, and say that the port they listen on cannot be read. Every proposal names its file and
key and what it could not carry; names are slugs, kept apart across files (`postgres`, `postgres-2`). Saving the chosen
ones makes new services and stacks whose first draft is the proposal, never a version; a name the project has already is
kept. MCP: `list_service_proposals` and `save_service_proposals` (saving services is person_only unless *Let automation
change agent permissions*; stacks are always taken).

Built (ORC-264): Project settings › Services › *Import draft* (`DFWRJ`) reads the checkout once as it opens and shows
the proposals grouped by their file, each with what was found, a line per key it could not carry with why, and Keep or
Skip; what a file holds but cannot be proposed, and a proposal whose name the project has already, show Skip with why.
*Save N drafts* saves the kept ones as they were read (a person saves here, so no widening question) and returns to the
tab; *Cancel* saves nothing. Not built: the Orchestrator chat's proposal cards.

## Limits and load

- Services at once: the app setting (computer) and each project's share.
- Test suites take a service slot while they run their own processes.
- Builds reuse warm workspaces (ORC-114) instead of building per service.

Built (ORC-248): the computer's *Services at once* is an application setting in `orchester.db` (default **8**,
1–32); each project's share is with its limits (default **4**, 1 to the computer's limit; a share above a limit
lowered later counts as the limit). Eight because a dev server or a database container takes about a core and a few
hundred MB to a GB, which leaves a usual 8-core, 16 GB machine room for the agents and their builds; four as many as
agents work at once by default, so two projects at their defaults fill the computer. Each running service holds one
slot, a stack one per service it starts, a test suite's own process one while it runs. A slot is an OS file lock in
the user's data folder, as a QA slot (`orchester-service-slots/slot-<n>.lock` for the computer's and
`projects/<project>/slot-<n>.lock` for the share; a slot is one of each), so every runner counts the same slots and a
dead runner holds none. A start takes all its slots before it starts anything; while too few are free it waits, its
run's Conversation notes `waiting_for_service_slots` (slots needed, the share, the limit) and its answer says
`waited_for_slots` once it got them. The starts of one project that wait go on in the order they began to; between
projects, whoever tries first takes the freed slots. A start that could never get its slots (more than the share or
the limit, counting the slots its step holds already) is refused at once. Slots are freed when a service stops, its
step ends, its run pauses or ends, and while its turn waits for a person. The QA app still takes the separate *QA apps
at once* slots until it is migrated to a service. Both limits are read and set through `update_project_limits`
(`service_slots`, `computer_service_slots`); the settings screens are agent testing task 19.

## Screens (design board `eWydp`)

| Screen | Node |
| --- | --- |
| Project settings › Services; empty; Runs › Limits row | `G1sdS`, `CdjWu`, `tJkyQ` |
| Flaky tests | `Tv9VC` |
| Service editor; Stack editor; Test suite editor; Import draft | `TGRTt`, `iNxTM`, `uxYcD`, `DFWRJ` |
| Task › Services; service crashed card; approve services card | `cjvRr`, `FwHrv`, `hvQDS` |
| Summary › Tests card | `qN2FK` |
| States: no Docker, macOS permissions, readiness timeout, slots busy | `oXVuM` |
| Settings › General › Services at once (the computer's total) | `UIVhQ` |
| Service editor · native driver (desktop-native) | `izC7e` |
| Editor states: database container, seed command, missing here, member read-only, delete modal, import again, Orchestrator proposal | `IWfVg` |
| Task › Services: waiting for you, full log, none yet | `iUTSx`, `Kd8zD`, `E1zah` |
| Conversation cards: run_tests, ui_* through mcp, approved by rule, denied, stopped | `C0dcl2` |
| Tests card: running, passed, lint failed, report unreadable, timed out | `je8z3` |
| Waiting for service slots in Tasks, the sidebar and the tab badge | `ewA0y` |
| Loading versions of every screen | `Chf6V` |

Built (ORC-265): the task page's fourth tab, Services (`cjvRr`; `orchester-app`'s `screens::task::services`). While
the run runs on a runner that is online, the page asks the runner every second for the services the steps at work
started (`Command::Services`, `orchester_ipc::services`, protocol 53): each one's state, port, URL, when it started and
ended, the stack whose start brought it up, who started it (the agent or the person) and the service whose crash
stopped it. The tab counts them with a dot (`danger` once one crashed, `info` while one starts, `success` while all are
ready), lists them with their UI driver and readiness from the published definitions, and shows the selected one's
log (the first that crashed, else the first) under its row, read every second after the last line read while
*Follow* is on; *Open full log* shows every kept line in a dialog. Log lines carry the time they were kept; their
secret values were replaced before the runner kept them. Restart, Start, Stop and *Stop all* go to the runner
(`Scheduler::services`): Stop stops a ready service as `stop_service` does and gives the start of a starting one up;
Restart stops it and starts it again as published with what it needs, approved by the person's press, without waiting
for an agent's start under way (a service both bring up at once is kept once). A URL opens in a new tab of the in-app
Browser. Not built: *Services at once* slots in the stack line, and the design's look of the `browser` driver's chip
(since ORC-256 it is the driver's name, as the others are). The QA seed's `run-services` scenario (with `-FakeAgents`)
holds real services for it.

Built (ORC-263): the stack editor (`iNxTM`) and the test suite editor (`uxYcD`) under Project settings › Services
(`orchester-app`'s `screens::project_settings::services::{stack, suite}`, the workspace's `agent_testing::
{stack_editor, suite_editor}`). Both save their draft as a field is left or a choice made, say the core's errors in
place and publish. A stack lists every service its start brings up in start order (`StartPlan::for_stack`), each with
its step and whether it was chosen or a need brought it, the *Services at once* slots it takes of the computer's, the
suites that need it, and its tree of needs (each service under the need it waits for longest). A suite's filter is the
one argument holding `{{filter}}`, edited in its own field and kept where it stood among the arguments; the switch the
design draws as *Re-run a failed test once* is the suite's `always` flag (*Run in every Test step*), since the flaky
re-run is the Test step's own; *Last runs* says no results are kept until the Test step stores them. A suite runs a
program, so an MCP client changes what it runs or publishes it only under *Let automation change agent permissions*;
a stack widens nothing. Not built: *Duplicate*.

Built (ORC-266): the Tests card on a task's Summary (`qN2FK`; `orchester-app`'s `screens::task::summary::tests`). A
Test step that runs to its end keeps a record beside its suites' runs in the run's storage
(`test-steps/<execution>.json`, `orchester_core::test_step::TestStepRecord`): each layer it reached or not, each
suite's first run with its totals and time, the failed tests with file:line, message, excerpt and artifacts, the flaky
tests (marked when quarantined), the quarantined tests that failed again, and the files the suites left. The task page
reads the latest execution's record (`orchester_workspace::tasks::test_results`), keeping only artifacts inside the
run's storage, and shows it under the run in place of that step's report among the output; a run without one has no
card. A link hands a picture, archive, video or text to its default program and any other file's folder (it could be
a program); a press of automation opens nothing, and a QA copy or a test records what it would open
(`services::links`). While the step the failed tests went back to works on them, the run's last event says how many
went back to which step. The seed's `summary-tests` scenario holds such a run. Not built: the other states of `je8z3`
(running, report unreadable, timed out), *Service logs* among the artifacts, and the stacks on a layer's line.

Built (ORC-275): the service cards of a run's Conversation (`orchester-app`'s `screens::task::conversation::timeline::
service`, `screens::task::cards::approval::services`, their parts in `ui::service_card`). The `service_crashed` note
is the crash card (`FwHrv`): the service, its exit code, when and how long after it started, what Orchester stopped
with it, its last lines and Orchester's own line, then *Restart*, *Open log* (the Services tab) and *Edit* (the
service's page). A start that fails because Docker is not available (missing, or its engine not answering) or a
service is not ready in time is recorded as a `service_start_failed` note (`orchester_core::service::
ServiceStartFailure`: the service, the cause with what readiness asked, its last 20 lines, the services not started
because of it and those the step still runs), shown as 10a and 10c of `oXVuM`; the `waiting_for_service_slots` note is
10d, its bar the computer's slots with the ones the start needs. A start of services that waits for the person is one
card for the service and its needs (`hvQDS`): each program in start order with whether a rule of the project approves
it as it is, changed it since (which parts) or never ran it, the slots it takes of the computer's, and *Approve*,
*Approve for every run while unchanged* (confirmed in a modal only a person confirms) and *Deny*, each sent to the
runner. Restart and Retry are a person's press only. The seed's `service-cards` scenario holds every card recorded,
`run-services-approve` (with `-FakeAgents`) a live start that asks. Not built: the root cause a crash names in its
title, other services' lines in a crash's tail, the times of its lines, the runs that hold slots and how many are free
(the notes carry none of them), and the start's working folder on the approve card.

## Phases

1. Model and storage of services, stacks and suites; report parsers.
2. Services in the runner (lifecycle, needs, readiness, logs, slots, approval) and the service tools.
3. `run_tests` and the Test step (layers, flaky re-run, failed tests back, stored result); quarantine.
4. `ui_*` with the `mcp` driver (the QA app as a service), then `browser` and `cdp`.
5. `native` on Windows, then on macOS.
6. Screens: settings, editors, import, the run's Services tab, the Tests card, flaky tests.
