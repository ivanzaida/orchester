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
   browser; the `browser` and `cdp` drivers below describe that later step and are not built now (ORC-256, ORC-260
   deferred).

## Concepts

### Service

A program Orchester starts for a run.

| Field | Meaning |
| --- | --- |
| program, arguments | No shell. `{{port}}`, `{{url}}`, `{{run}}` and `{{services.<name>.url|port}}` are filled per run |
| working folder | Inside the run's worktree |
| variables | Plain values, or a project secret by name (never shown, never logged) |
| needs | Services started first, in order; each must be ready before the next |
| readiness | The port accepts connections; an HTTP 2xx/3xx on a path; or a log line matching a pattern. Plus a timeout; on timeout the start fails with the last log lines |
| seed | Runs once after ready: a sequence of HTTP requests whose answers may be captured (`team_id ← $.id`) for later requests and for agents, or a command |
| per-run data | None; a fresh database container; or files in the run's own folder. Deleted when the run ends |
| UI driver | `none`, `browser`, `native`, `cdp` or `mcp` (see [UI drivers](#ui-drivers)), and where `ui_open` starts |
| owned paths | Repository paths (`web/cloud/**`); a run that changed them starts this service in its Test step |
| approval | The first start asks a person; *Approve for every run* keeps answering while the program, arguments, variables and seed are unchanged |

Ports and data are per run, so parallel runs never clash. A service lives for the step that started it: Orchester stops
it when the step ends, when the run pauses or fails, and while the run waits for a person (as QA apps today), and
releases its slot. A crash is reported to the agent as an event with the exit code and the last log lines, and to the
person in the Conversation.

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
| `service_status(name)`, `service_logs(name, tail, since)`, `stop_service(name)` | State, log tail (secrets redacted), stop without asking |
| `run_tests(suite, filter?)` | Runs a suite (starting what it needs) and returns the structured result: counts, each failure with message, file:line, a short excerpt and its artifacts |
| `test_output(run, test)` | One test's whole output, when it is really needed |
| `ui_open(service)`, `ui_snapshot()`, `ui_act(ref, action)`, `ui_screenshot()`, `ui_logs()`, `ui_wait(condition)` | See and use the service's UI through its driver |

Agents are told never to start servers or test runners by shell. `start_qa_app` and `call_run_app` become the
`desktop-qa` service with the `mcp` driver.

## UI drivers

| Driver | Snapshot and act | Screenshot | Logs |
| --- | --- | --- | --- |
| `browser` | A headless browser per run over CDP: accessibility tree with refs, actions by ref; it may open only the run's own service origins | The page | Console errors and failed requests |
| `cdp` | Electron and WebView2 apps through their remote debugging port | The window's web content | Console |
| `native` | Windows UI Automation; macOS Accessibility (AX). Actions through element patterns (invoke, value, toggle, scroll), never global input | Only that window (Windows: the window's capture; macOS: ScreenCaptureKit by window id) | stdout/stderr |
| `mcp` | Apps with their own MCP (lurq, Orchester): the existing relay | Through the app | Through the app |

Rules for every driver: only processes the run started (as `stop_run_process` checks), never global input, never a
whole-screen capture (AGENTS.md rule 16), windows start off-screen and without focus where the OS allows it. An
element without the needed pattern answers an explicit *unsupported for this element*. On macOS the native driver
needs Accessibility and Screen Recording; without them it shows a visible state naming the missing permission and where
to grant it. Platform code lives in small `cfg(target_os)` modules; FFI with `unsafe` in dedicated modules (rule 9).

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

## Import

A first draft of services and stacks can be read once from `docker compose` files, `package.json` scripts and Cargo
binaries; the person keeps or skips each and edits the drafts. The Orchestrator chat may propose services as cards the
person applies (as MCP servers today), since a service runs a program.

## Limits and load

- Services at once: the app setting (computer) and each project's share.
- Test suites take a service slot while they run their own processes.
- Builds reuse warm workspaces (ORC-114) instead of building per service.

## Screens (design board `eWydp`)

| Screen | Node |
| --- | --- |
| Project settings › Services; empty; Runs › Limits row | `G1sdS`, `CdjWu`, `tJkyQ` |
| Flaky tests | `Tv9VC` |
| Service editor; Stack editor; Test suite editor; Import draft | `TGRTt`, `iNxTM`, `uxYcD`, `DFWRJ` |
| Task › Services; service crashed card; approve services card | `cjvRr`, `FwHrv`, `hvQDS` |
| Summary › Tests card | `qN2FK` |
| States: no Docker, macOS permissions, readiness timeout, slots busy | `oXVuM` |

## Phases

1. Model and storage of services, stacks and suites; report parsers.
2. Services in the runner (lifecycle, needs, readiness, logs, slots, approval) and the service tools.
3. `run_tests` and the Test step (layers, flaky re-run, failed tests back, stored result); quarantine.
4. `ui_*` with the `mcp` driver (the QA app as a service), then `browser` and `cdp`.
5. `native` on Windows, then on macOS.
6. Screens: settings, editors, import, the run's Services tab, the Tests card, flaky tests.
