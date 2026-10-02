The limit payloads reproduce the samples in the ORC-17 handoff and
`docs/design/orchester-workspace.md`, “Data sources”, in the form Orchester reads them.

`codex/plan-limits.jsonl` is a Codex App Server `account/rateLimits/updated` notification,
shaped by the v2 schema `codex app-server generate-json-schema` writes (codex-cli 0.159.2,
`AccountRateLimitsUpdatedNotification`); its percentages are illustrative.
`claude/plan-limits.jsonl` is the `rate_limit_event` Claude Code 2.1.89 printed with
`--output-format stream-json`, as supplied in the task's verified Data sources note (reset
normalized). These fixtures contain only limit metadata, never prompts or credentials.
