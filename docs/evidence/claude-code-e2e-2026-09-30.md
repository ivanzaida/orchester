# Claude Code end-to-end check, 2026-09-30

One short task ran through the runner with the real Claude Code CLI on `claude-opus-5-5`. The
pipeline had two steps, Implement then Review, both on one Claude Code profile (effort `low`,
Write access, **Ask before running commands**, network off, which Claude Code cannot enforce and
leaves on without web tools). The branch was `claude/claude-support`, after commits `6458c3c`,
`61cde87` and `32d5b25`.

## Setup

- Harness: a scratch runner integration test (not committed; kept at
  `.tmp/claude-e2e/claude_e2e_scratch.rs`). It builds the project with the runner test support,
  then points its connection at the real CLI and records a real connection check. It publishes one
  project skill (`readme-edits`) and runs the task through `Scheduler`. The scheduler used the
  worktree's debug `orchester-runner.exe` as the permission prompt relay. `TMP`/`TEMP` pointed at
  `.tmp/claude-e2e/tmp`, so the repository, worktrees and private MCP files stayed there
  (repository `.tmp/claude-e2e/tmp/.tmpanP0UI`).
- The harness played the person. It approved the first request if it was exactly
  `git tag claude-e2e-approved` and would have denied anything else. Only that one request came.
- Task brief: append one line `hello` to `README.md`. Before the edit, run exactly
  `git tag claude-e2e-approved` with Bash and no other command. Acceptance criterion: "README.md
  ends with hello". Review instructions: read `README.md` and run no shell command.

## First attempt: Claude Code 2.1.89 cannot use Opus 5.5

With the user's CLI on `PATH` (`C:\Users\lurkm\.local\bin\claude.exe`, 2.1.89), the first turn
ended with this API error. It was recorded as `TurnFailed`, and the step failed visibly:

```text
API Error: 400 ... "Claude Code 2.1.89 does not support this model; version 2.1.280 or newer is
required. Run 'claude update', or update the Claude desktop app, then try again."
"error_code":"claude_code_version_too_old"
```

The CLI was not updated: that would change the user's install. The run was repeated with the
newer CLI the Claude desktop app had already installed,
`C:\Users\lurkm\AppData\Roaming\Claude\claude-code\2.1.284\claude.exe`. It uses the same signed-in
account (`claude auth status --json`: logged in, `claude.ai`). No sign-in or configuration change
was made.

## Connection check (2.1.284)

`check()` ran `--version`, `auth status --json` and the model list request (the stream-json
`initialize` control request, no prompt, no session). Result: `2.1.284`, signed in, catalog source
`ProviderAndCurrentIds`. The first models are `claude-opus-5-5`, `claude-sonnet-5-5`,
`claude-fable-5-1` and `claude-haiku-4-5-20251001`. The CLI's own list follows: `opus`, `sonnet`,
`haiku`, `claude-sonnet-5`, `claude-opus-5`, `claude-fable-5`, `claude-opus-4-8`, … Opus 5.5 got
`low, medium, high, xhigh, max`; 2.1.284 accepts `xhigh`, and 2.1.89 does not.

## Command line

The command line was read from the process table while the approval waited. The approval token is
only in the private approvals file, not in the command line.

```text
"C:\Users\lurkm\AppData\Roaming\Claude\claude-code\2.1.284\claude.exe" -p --output-format stream-json
  --verbose --model claude-opus-5-5 --effort low --strict-mcp-config --setting-sources ""
  --disable-slash-commands
  --mcp-config H:\projects\orchester\.tmp\claude-e2e\tmp\orchester-mcp\<project>\<invocation>-approvals.json
  --permission-prompt-tool mcp__Orchester__approve
  --tools Read,Glob,Grep,TodoWrite,Edit,Write,NotebookEdit,Bash --permission-mode acceptEdits
  --disallowedTools WebFetch WebSearch
  --add-dir H:\projects\orchester\.tmp\claude-e2e\tmp\.tmpanP0UI\.orchester\skills\<execution>
  --session-id 01a0f3d8-0518-7500-b224-fdc5451f95af

H:\projects\orchester\.tmp\worktrees\claude-claude-support\target\debug\orchester-runner.exe --claude-permission-prompt
```

The second line is the relay that Claude Code started from the approvals file. `Bash` is offered
without prefix rules, because a person decides each command.

## Prompt parts

Claude Code stores the prompt it received in its session transcript
(`~/.claude/projects/<worktree>/01a0f3d8-0518-7500-b224-fdc5451f95af.jsonl`, first user message,
3,043 characters). It has these sections in order: `# Your role: Builder`,
`# Current step: Implement`, `# Skills` (listing `readme-edits` with its `SKILL.md` path in the
`--add-dir` folder), `# Task ORC-1: Say hello` with `## Acceptance criteria`, and
`# When you finish` (the step result contract). The agent's first tool call read that `SKILL.md`
through the `--add-dir` folder.

The internal code-index skill, the code index folder given with `--add-dir`, and the memory parts
are not on `master`. They are on `claude/integration`, so they were not tested.

## Trimmed event log (Implement, then Review)

```text
SessionStarted { session: "01a0f3d8-0518-7500-b224-fdc5451f95af" }
CommandStarted Read …\skills\<execution>\readme-edits\SKILL.md
TurnCompleted { input: 9217, cached_input: 5435, output: 16 }
CommandStarted Read …\worktrees\<run>\README.md          -> "1\thi"
CommandStarted { item: "toolu_019Wjg4Q1vzoEJcP9qMoT9j5", command: "git tag claude-e2e-approved" }
  approval requested: command, action "git tag claude-e2e-approved", scope "Create approval tag",
  target = the worktree; the harness approved it after 0.5 s
Raw rate_limit_event {"status":"allowed","resetsAt":1790812200,"rateLimitType":"five_hour",
  "unifiedWindows":{"five_hour":{"utilization":0.02,…},"seven_day":{"utilization":0.1,…}}}
CommandFinished git tag claude-e2e-approved -> "(Bash completed with no output)", Completed
TurnCompleted { input: 9737, cached_input: 9215, output: 21 }
FileChanged README.md Modified
Message "…{"outcome":"done","summary":"Ran `git tag claude-e2e-approved` (the only shell command), then added a line `hello` …"}"
TurnCompleted { input: 10050, cached_input: 9735, output: 157 }
TurnCompleted { input: 0, cached_input: 0, output: 477 }          (the result event's remainder)
SessionStarted { session: "01a0f3d8-3ea5-73ea-a98f-1df22c580dd8" }  (Review)
CommandStarted Read README.md -> "hi\nhello"
Raw rate_limit_event {… same shape …}
Message "…{"outcome":"done","summary":"Reviewed the uncommitted README.md … ends with `hello` …"}"
TurnCompleted …
```

## Results

| Check | Result |
| --- | --- |
| Run | `Succeeded`, 24 s. Branch `orchester/orc-1-run-1` commit `73c026b` has `README.md` = `hi\nhello`. Tag `claude-e2e-approved` exists. |
| Model | Both invocations record `claude-opus-5-5`, and `--model claude-opus-5-5` reached the CLI. |
| Approval round trip | One request (`<invocation>:toolu_019Wjg4Q1vzoEJcP9qMoT9j5`). **Approve once** returned `{"behavior":"allow","updatedInput":{}}`. The command then ran once with its own input. 2.1.284 kept the original input. |
| Waiting time | Implement `inactive_ms` = 1212: the wait for the decision was not active time. |
| Tokens | Implement: input 29,004, cached 24,385, output 671, counted 5,290. Review: input 19,064, cached 16,521, output 277, counted 2,820. Run used 8,110. |
| Rate limit | Each `rate_limit_event` is kept as a raw activity line. 2.1.284 nests its fields under `rate_limit_info` (`status`, `resetsAt`, `rateLimitType`, and per-window `utilization` in `unifiedWindows`). Orchester does not parse it into a structured event; Codex's rate limits are not parsed either. |
| Tokens before first edit | **Not on master.** `EditProgress` lives on `claude/integration`. By the log, the first edit was made in the second turn; the two turns up to it used 18,954 input, 14,650 cached and 37 output tokens. Master records no such figure. |
| Structured result | **Not parsed.** Both final messages put prose before the JSON object, although the prompt says "End with exactly one JSON object". `StepResult::parse` accepts only a message that is exactly the JSON document. So both outputs have `outcome: None` and the whole message as text. The steps passed on the CLI's exit (`Completed`). A `question` or `failed` written this way would not be recognized. See "Not done". |

## Not done

- The result is not structured the way Codex's is. Codex gets `--output-schema`. Claude Code has
  `--json-schema` (its `result` event then carries `structured_output`), but that was not verified
  with a model turn. It takes inline JSON, which an npm `.cmd` launcher could refuse
  (`UnsafeScriptArgument`). It is left as a follow-up in `docs/plan.md`.
- Claude Code 2.1.89, the CLI on the user's `PATH`, cannot run Opus 5.5 (see above). Running it
  needs `claude update` or pointing the connection at a CLI of version 2.1.280 or later.
- Denial, cancel, pause and stale decisions were covered by the fake-CLI runner tests
  (`crates/runner/tests/claude_approvals.rs`), not with the real CLI.
