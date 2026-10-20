The limit payloads reproduce the samples in the ORC-17 handoff and
`docs/design/orchester-workspace.md`, “Data sources”, in the form Orchester reads them.

`codex/plan-limits.jsonl` is a Codex App Server `account/rateLimits/updated` notification,
shaped by the v2 schema `codex app-server generate-json-schema` writes (codex-cli 0.159.2,
`AccountRateLimitsUpdatedNotification`); its percentages are illustrative.
`claude/plan-limits.jsonl` is the `rate_limit_event` Claude Code 2.1.89 printed with
`--output-format stream-json`, as supplied in the task's verified Data sources note (reset
normalized). These fixtures contain only limit metadata, never prompts or credentials.

The reads outside runs (ORC-55): `codex/rate-limits-read-signed-out.jsonl` is what `codex app-server` 0.137.0 printed
on Windows on 2026-10-20 for `initialize`, `initialized` and `account/rateLimits/read` under an empty scratch
`CODEX_HOME` (nobody signed in; the server name and installation id replaced). `codex/rate-limits-read.jsonl` is the
same session signed in, its answer shaped by the schema `codex app-server generate-json-schema` writes for that
version (`GetAccountRateLimitsResponse`, with a second bucket of one model in `rateLimitsByLimitId`); no real
account was read, so its numbers are illustrative. `claude/check-now.jsonl` and `claude/check-now-signed-out.jsonl`
are *Check now*'s `stream-json` output shaped like `reviewer/claude-review.jsonl` and the CLI's sign-in refusal; no
real account ran them either.
