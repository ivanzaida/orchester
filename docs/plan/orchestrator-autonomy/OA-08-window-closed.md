# OA-08 · Agents keep working with the window closed and the computer awake

- **Status:** todo (the person, 2026-11-13: only keeping the computer awake)
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
