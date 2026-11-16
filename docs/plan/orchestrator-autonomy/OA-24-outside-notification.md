# OA-24 · A notification that leaves the computer

- **Status:** canceled (the person, 2026-11-13: nothing leaves the computer)
- **Kind:** design + app
- **Depends on:** OA-15 for what it tells
- **Branch:** —
- **Tokens:** —

## Goal

A person who is away learns that their module stopped and needs them, without sitting at the computer that runs it.

## Today

OS notifications in the groups *Needs you*, *Stopped* and *Finished* (`crates/core/src/notification.rs`); what the
Orchestrator did itself raises none. Nothing leaves the computer.

## Proposed

- **Settings › Notifications › Send to a webhook:** off by default. One address the person types; it usually holds a
  token, so it is kept in the OS secret store, shown masked, and never logged.
- **What is sent**, each kind chosen by the person: a task was parked for you; what a conversation leads is stuck; a
  module is done; an event was left to you after its tries (OA-01); the daily report is ready.
- **The body** is Orchester's own words in one small JSON object: the kind, the project's name, the task's key and
  title, what is needed in the words of the kind, the time, the `orchester:` link. Never an agent's text, a reason,
  a brief, a file's name or a secret: what an agent wrote may carry anything, and this leaves the computer.
- A POST with a short timeout, three tries; a failure shows in Settings beside the row and never holds the work.
- *Send a test* in Settings.

## To decide

- Whether anything may leave the computer at all, and that task titles are acceptable in it.
- A plain webhook (which a phone push service, a chat's incoming hook or the person's own script can take) or one
  named service. A plain webhook needs no account of Orchester's and no new dependency if the app's HTTP client is
  enough; say so in the task once decided.

## Done when

Set after the decision.
