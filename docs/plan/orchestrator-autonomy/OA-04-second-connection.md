# OA-04 · A second connection for the Orchestrator

- **Status:** later (the person, 2026-11-13: not now; the chat waits for the reset)
- **Kind:** design + UI + runner
- **Depends on:** OA-03
- **Branch:** —
- **Tokens:** —

## Goal

When the connection the Orchestrator runs on cannot start a turn (its account is at its plan limit, or signed out),
turns Orchester starts go on with a second connection the person chose, instead of waiting for hours.

## Today

`OrchestratorSettings` holds one connection, model and effort (`crates/core/src/orchestrator/settings.rs`). A turn on
another connection or account starts a new session and the conversation says so (`NewSession`). The accounts design
left a fallback order of accounts for later (`docs/design/agent-accounts.md`, decision 3).

## Proposed

- Orchestrator settings › Profile gets an optional *When it cannot start, go on with* row: a connection, model and
  effort, chosen with the compact *Runs on* picker.
- It is used only for turns Orchester starts by itself and only while the first is held (OA-03) or the person's to
  fix; the first is tried again at its reset. A person's own message stays on the first and says why it cannot start.
- The conversation names the change of session each way, as it does today.

## To decide

- Whether to build it at all, or to wait for the reset (the cheaper answer, and OA-03 already holds the events).
- Whether the second connection may be of the other CLI (Claude Code ↔ Codex): the session, the tools the agent reads
  the repository with and the confinement differ (`docs/design/orchestrator-toolset.md`, decision 10).

## Done when

Set after the decision.
