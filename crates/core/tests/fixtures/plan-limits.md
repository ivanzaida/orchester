The limit payloads reproduce the Codex session and Claude Code stream-json samples in
the ORC-17 handoff and `docs/design/orchester-workspace.md`, “Data sources”. Claude's
sample was observed on 2.1.89. The surrounding Codex session envelope and timestamps
are normalized for deterministic tests; the Codex percentages are illustrative.
These fixtures contain only limit metadata, never session prompts or credentials.

`codex/captured-plan-limits.jsonl` contains an actual Codex session's limit payload
captured on 2026-09-30. Only its envelope timestamp is normalized; usage, credits,
window, reset and plan fields are unchanged. Claude's payload is the observed
2.1.89 event supplied in the task's verified Data sources note (reset normalized).
