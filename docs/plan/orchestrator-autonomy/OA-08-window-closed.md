# OA-08 · Agents keep working with the window closed and the computer awake

- **Status:** done; its row has no frame yet (2026-11-14)
- **Kind:** app (Windows and macOS) + UI
- **Depends on:** the frame of its setting (`frames.md` F) for the visible part
- **Branch:** —
- **Tokens:** —

## Goal

Closing the window, or a computer that goes to sleep, does not stop what the person left the Orchestrator to do.

## Today

The follower runs in the app's process with each open project's feed (`crates/desktop-app/src/project/feed.rs`), and
"the runners and their agents do not outlive the window" (`crates/desktop-app/src/lifecycle/mod.rs`). There is no
tray or menu-bar presence, and nothing keeps the computer awake.

## Research first

Written to `.tmp/orchestrator/orchestrator-autonomy/OA-08-map.md`:

- what closing the last window does today, step by step, and what *Quit now* does;
- whether lurq can keep the process alive without a window and show a tray icon (Windows) and a menu-bar item
  (macOS), or what it lacks;
- how to ask the system not to sleep while work is under way on both (a power request on Windows, a power assertion
  on macOS), and what a closed laptop lid does to each.

## Proposed

- **Settings › General:** *Keep working when the window is closed* and *Keep the computer awake while agents work*,
  both off until the person turns them on.
- With the first on, closing the last window leaves the app running while a project drives and has work under way,
  with a tray or menu-bar item that says so, opens the window and quits. With nothing under way the app quits as
  today.
- With the second on, the app holds the system awake only while an agent works or a turn thinks, and lets go at once.
- Platform code behind `cfg(target_os = …)` in small named modules, both platforms in the same change (`AGENTS.md`
  rule 0); FFI only in a dedicated `unsafe` module if no safe wrapper exists (rule 9).

## Decided (2026-11-13)

Only the second setting is built: *Keep the computer awake while agents work*. Keeping the app alive without its
window is not built; closing the window stops the work as today, and OA-19 says so before the person leaves. The
research keeps only its third question.

## Done when

- The rule "work is under way" (an agent works or a turn thinks) is the workspace's, tested there; the platform
  modules only hold and release, each named in the report with the macOS check listed as not run on Windows.
- The setting's row matches its frame (an independent design verification). `tools/check.ps1` passes.

## As built (2026-11-14)

- **The rule** is the workspace's (`orchester_workspace::awake::work_under_way`): work is under way in a project
  while a run of it runs with no step waiting for a person (the test a team claim's renewal uses,
  `orchester_core::run::at_work`), or a turn of its Orchestrator chat thinks. A run that waits, is paused,
  stopped at a limit, interrupted or ended is not at work.
- **The hold** is the app's (`crates/desktop-app/src/awake.rs`): each project's follower looks after every change of
  its runs and conversations, and the app holds the system awake while any open project works and the setting is on,
  and lets go at once otherwise. Nothing polls. Turning the setting off lets go at once.
- **The platform modules** only hold and release (`crates/exec/src/awake/`):
  - Windows, `windows.rs`: `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED)` on a thread the hold owns,
    since the state belongs to the thread that set it. It is the one `unsafe` module added (listed in `AGENTS.md`).
  - macOS, `macos.rs`: the system's `/usr/bin/caffeinate -i -w <pid>` as a child, killed when the hold is dropped
    and ending by itself with Orchester. No FFI. **Not run on a Mac**: the macOS check is not run from Windows.
  - Any other system answers *not supported*, which the app logs once.
- **The setting** is the app's (`app_settings.keeps_awake`, off until the person turns it on), with a row in
  Settings › General, in a group *Work*, built as *Send error reports* is. Its own frame (`frames.md` F) is not
  drawn, so no design verification was possible.

What it does not do, as decided: closing the window still stops the work, and a laptop still sleeps when its lid is
closed on battery power (that sleep is not an idle sleep, and neither platform call holds it).

The research file the task asked for was not written: only its third question was left, and its answer is the two
platform modules above.
