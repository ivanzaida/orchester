# Orchestrator handover (Claude → Codex), 2026-10-11

You take over as the orchestrator of Orchester's own development. The orchestrator does not write product code.

The goal is to make the new app (`crates/desktop-app`, binary `orchester-app`, design `data/orchester2.pen`) good
enough to develop Orchester inside Orchester. The work happens in the project **Orchester** inside the installed app.
There, Codex agents run tasks through pipelines. You:

- watch the runs;
- answer the agents' questions within the person's decisions;
- return failed steps with concrete notes;
- have every finished branch reviewed by someone who did not write it, and get its findings fixed;
- ask the person before every merge.

Read `AGENTS.md` first; everything in it applies. The person writes in Ukrainian; report to them in Ukrainian, short
and concrete.

## Latest handover from Claude (2026-10-13) — stable MCP connection

The person is fed up with how clients find the app's MCP server. A Claude desktop session cannot find the
installed app at all (see "A Claude session cannot reach the app's MCP" below). The person decided to replace the
discovery file with a stable address and token, set up in *Settings › Automation*.

**Design.** Committed in `433529d5`:

- The note `docs/design/app-mcp-connection.md`. Read its *Decisions* section.
- The frames are in the Settings layer `uyO5v` of `data/orchester2.pen`:
  - `lye6f`: Settings › Automation, the MCP server;
  - `NzOSY`: Connect an agent with Codex, scopes and macOS;
  - `eM8tB`: rotating the token in place, and showing it;
  - `eTrCO`: the server's states;
  - `QqBCj`: the status bar's MCP item and its popover.
- Screenshots are in `.tmp/design-mcp-settings/`.

**The person's decisions, 2026-10-13:**

- The address is `127.0.0.1:47821/mcp`. When the port is taken, the app says so and does not move to another port.
  Debug builds and QA copies stay as they are today, with a free port and a discovery file. For QA copies and tests,
  that file goes into the worktree's `.tmp/`.
- The token lives in the OS secret store, one per installation.
- *Copy* clears the clipboard after 60 s.
- Claude Code connects with `claude mcp add --scope user`.
- Codex connects through the environment variable `ORCHESTER_APP_MCP_TOKEN`
  (`--bearer-token-env-var`); the `config.toml` entry is the alternative.
- The server is named `orchester`, in lower case.
- Header helpers and naming the program that holds a taken port come later.
- Still open: whether *Agents connected* is kept only in memory. Build it in memory only and say so.

**The task is not created yet.**

- A ready `create_task` argument file is at `.tmp/setup-tasks/app-mcp-connection.json`: pipeline *Orchester UI
  change*, priority high.
- First check with `list_tasks` whether the person already created it. The title starts with *Stable MCP address and
  token*. If they did, use it and do not create a duplicate.
- Create it if it is missing.
- **Do not start it without the person's word**: the rule against starting tasks on your own still holds.
- Once it is merged and the person has reinstalled the app, a Claude session reconnects once with the `claude mcp
  add` command shown in Settings.

## Earlier handover from Claude (2026-10-12) — agent accounts

**The person asked** (2026-10-12): *"створи в оркестрі таску і почни її відстежувати"*. That means: create the agent
accounts task in the Orchester project, start it, and track it. This explicitly authorizes `run_task` for this one
task only. The rule against starting other tasks on your own still holds.

0. Update 2026-10-13: it exists as **ORC-26** and was not started yet; the project database shows no run.
   Following the person's request quoted above, start it and track it.
1. First check whether the person already created it themselves. Run `list_tasks` and look for the title *Agent
   accounts: several Claude Code and Codex accounts per computer*; it would probably be ORC-24. If it exists, use it
   as it is and do not create a duplicate.
2. If it does not exist, create it from `.tmp/setup-tasks/agent-accounts.json`. That file is a ready `create_task`
   argument object: pipeline *Orchester UI change*, priority high, ten criteria. Then `run_task` it.
3. Track it like the other runs. It is large, and the brief asks the implementer to commit in slices (domain and
   storage, runner invocation, plan limits per account, Settings › Accounts, connection field and run display,
   limit-reached choice), and to say exactly what is left if it cannot finish in one run.

**What the task is built from:**

- The design note `docs/design/agent-accounts.md`, committed in `fe8feb8f`.
- The frames in `data/orchester2.pen`:
  - root layer "Accounts" `hgOl2`;
  - components layer "Accounts · components" `Cgob0`, with `I6Ebu`, `LW6D5` and `B9myCE`.
- Screenshots of the frames are in `.tmp/design-accounts/`.
- The person must keep Pencil open with `orchester2.pen` saved; the implementer and the verifier read it through
  `pencil-design`.

**The person's decisions (2026-10-12), also in the note's *Decisions* section:**

1. Accounts are app-wide (*Settings › Accounts*) and chosen by the connection.
2. Sign-in happens in a visible terminal that runs the CLI's own login, with the command also shown to copy.
3. Limit reached:
   - never move a running agent to another account;
   - wait for the reset by default, or continue on another account explicitly, as a new session;
   - a fallback order of accounts comes later.
4. Removal is careful:
   - block it while a run uses the account;
   - never delete a folder the person brought;
   - move Orchester's own folder to the Recycle Bin only when the person ticks it.

**Still open**, for v1 the defaults the designer proposed:

- a new account starts with empty settings;
- runs and MCP show the account name, not the email;
- no GitHub accounts;
- folders the person already has;
- the same account signed in twice;
- Claude accounts on macOS, until Keychain behaviour is checked on a Mac.

The person did not accept the designer's recommendations for the first three. Ask them before anything beyond v1
depends on these.

**A Claude session cannot reach the app's MCP** (seen 2026-10-12). The installed app (pid 11348, started 14:56)
wrote `C:\Users\lurkm\AppData\Local\orchester-app-mcp\11348.json`; the person confirmed this in Windows. A Claude
desktop session sees a different, stale content of that folder: only `40908.*` from 2026-10-10. It sees the same
inside and outside its sandbox, even though it sees `E:` and `H:` current.

The likely cause is the Claude desktop app's MSIX AppData virtualization; this is not verified. Codex is not affected.

If Claude orchestrates again, it needs one of these:

- the person copies the discovery file into `H:\projects\orchester\.tmp\app-mcp\`, and Claude calls the client with
  `--discovery-dir .tmp/app-mcp`;
- an app setting or environment variable for the discovery folder. This was proposed to the person and not decided.

Master has moved on since the snapshot below. Most recently, ORC-5 Skills was merged (`2fef3eb3`). Use
`.tmp/orchester/watch-state.json` and the live run states over the table further down.

## Standing decisions of the person

### Current confirmation delegation (2026-10-11)

The person explicitly asked: “зберіг, давай заапдейтимо те що забороняє тобі самому це робити”.
The orchestrator and independent verifiers may handle an individually reviewed confirmation within the authorized
task when the app/tool explicitly permits automation. Do not request a human click solely because this document
previously required manual UI checks. Record whether an outcome was produced by the person or by permitted automation;
an automated outcome is never reported as a human outcome. Preserve independent native/Pencil verification and
read back the actual result before continuing.

An actual `person_only`, classifier or tool refusal still requires the person. This delegation does not permit
retrying a refused action through another tool, agent, endpoint, direct database write or changed protection code.
Person-only switches must still be changed by the person. Keep protected pending states without agent input until
the actual person responds. Read `.tmp/orchestrator/watch-state.json` for current authorization and exact run state;
the older decisions and task snapshots below are historical when superseded by a direct instruction from the person.

- **Do not start new tasks (`run_task`) on your own** (2026-10-11). When a slot frees, say so and wait. Keeping
  existing runs moving is fine: answers, `continue_run` returns, fixes on finished branches, reviews.
- **Merge to master and (re)install the app only with the person's explicit OK**, per branch. Commit only when asked.
- **Do not answer approval cards in bulk or by script.** A script that auto-approved one QA client was refused as
  unsafe. Approve single requests only when the person asked you to, or when they already approved exactly that kind
  of request. Leave commands that run agent-written code outside the sandbox to the person unless they say otherwise.
- **The installed app** in `E:\projects\orchester` is the person's own. Never write to it, build into it or stop its
  processes. `tools/install.ps1 -InstallDir 'E:/projects/orchester'` runs only when asked. Use forward slashes:
  backslashes once mangled the path into `E:\projectsorchester`. Back up `orchester.db` together with its `-wal` and
  `-shm` files first.
- **Widening agent permissions over MCP** needs the person-only switch *Let automation change agent permissions* in the
  window. Only the person flips it. Changing an existing task's pipeline is person-only too.
- **The old app (`crates/desktop`) is dead**: use it for code lookup only. No QA in it, no features, no workarounds
  through it.
- Read `.pen` files only through the Pencil MCP. Never change the person's design files (`orchester2.pen`, and never
  `kontur.pen`).
- No modals (new/import are pages, confirmations sit in place). Token and time limits are optional, and none is the
  default. Everything must work on Windows **and** macOS.
- When Claude Code's safety classifier or a sandbox refuses something, do not work around it; tell the person.

## Driving the app

The app was **not running** when this was written: the person closed it, and the runs below continue when it and its
runner start again. Ask the person to start the installed app. Then find its pid from
`%LOCALAPPDATA%\orchester-app-mcp\<pid>.json`; it was 40908 before.

Call a tool:

```bash
python tools/lurq-mcp-client.py <pid> <tool> --app orchester-app --discovery-dir "$LOCALAPPDATA/orchester-app-mcp" --arguments '<json>'
```

Never print the discovery token. `"project":"Orchester"` works everywhere.

The tools you need, with their argument shapes:

| Tool | Arguments |
|---|---|
| `list_tasks` | project |
| `get_task` | project, key |
| `create_task` | project, title, brief, criteria[], pipeline, priority, workflow_status |
| `update_task` | project, key, workflow_status: backlog / todo / in_progress / in_review / done / canceled |
| `run_task` | project, key; returns the run id |
| `get_run` | project, run: steps with attempt and state, `status.reason`, `awaiting.approvals` / `questions` |
| `send_message` | project, run, text: answers a waiting question, or is delivered after the agent's turn |
| `continue_run` | project, run, choice; only when the run waits for a person (e.g. `retries_exhausted`); a succeeded run cannot be continued |
| `list_approvals` / `decide_approval` | project, approval, decision: approve_once / deny |
| `cancel_run`, `pause_run`, `resume_run` | |
| `get_profile` / `save_profile_draft` / `publish_profile` | profiles; `save_profile_draft` needs role, responsibilities, instructions, connection, model, effort, mcp_servers; leave permissions out to keep them |
| `get_pipeline` / `save_pipeline_draft` / `publish_pipeline` | pipelines |
| `list_models`, `list_tools`, `list_connections`, MCP-server tools | setup |

`continue_run` choices:

- `{"kind":"return_to","step":"implement","note":"…"}`
- `{"kind":"allow_retry"}`
- `raise_limits`, `raise_step_budget`, `resume`

The domain tools cannot read an agent's last message, a step's result or a question's full text. Read them from the
project database **read-only** (`sqlite3` URI `file:.orchester/orchester.db?mode=ro`) and never write it. The tables:

- `messages` (`run_id`, `author`, `kind`, `body`): agent questions and replies.
- `step_outputs` (`run_id`, `step_key`, `outcome`, `text`): each step's result.
- `run_events`: the sequence of events.

Verify reports also land in the run's worktree under `.tmp/verify/<name>/report.md`. Worktrees are at
`.orchester/worktrees/<run-id>`.

Watcher: `python .tmp/setup-tasks/watch_runs.py <pid> 60 <run-id>…` exits on the first notable change, such as a step
waiting, the run stopping, or a card for the person. Older helpers are in `.tmp/setup-tasks/`.

## Project setup (Orchester project, key ORC)

**Connections:**

- Codex · Orchester: `C:\nvm4w\nodejs\codex.cmd`, 0.159.2.
- Claude · Orchester: 2.1.286.

**Profiles.** All three run on Codex with `gpt-6.1-sol`, effort high:

- **Implement**: writes; asks before tool calls; has `pencil-design`.
- **Review**: read-only.
- **Verify**: has `pencil-design`.

On 2026-10-11 the instructions changed as follows:

- Implement and Verify do not run `tools/check.ps1`, because the Check step does.
- Verify uses the new Pencil tool names.
- Verify lists person-only states and states without a design frame as Unverified instead of failing on them.

The script that made these edits is `.tmp/setup-tasks/no_check_in_steps.py`.

**Pipelines:**

- *Orchester change*: Implement → Check → Review.
- *Orchester UI change*: Implement → Check → Review → Verify.

Check, Review and Verify return to Implement on failure, at most 2 times per step. After that the run waits for the
person with `retries_exhausted`.

**Check tool:** `powershell -NoProfile -ExecutionPolicy Bypass -File tools/check.ps1 -Tests changed`, timeout 10800
(since 2026-10-19 with the tests of the changed crates; `docs/stable-build.md`, Embedded MCP server).

**MCP server `pencil-design`:**

- Program: `D:\Pencil\resources\app.asar.unpacked\out\mcp-server-windows-x64.exe`.
- Arguments: `--app desktop --agent codexCLI`.
- Exempt tools: `execute`, `get_app_state`, `get_style`, `read_skill`.
- Agents may use `execute` only for read operations.

**Limits:** daily budget 40M tokens, 4 agent slots, no run limits.

## State of the work

Master is at `a7b0520e` (the ORC-21 merge).

Uncommitted on master:

- `docs/plan.md`: four follow-ups added under A5 by Claude:
  - Codex ends its turn while a tool call waits (now ORC-22);
  - Codex steps that don't ask before tool calls decline MCP tools;
  - program-place gaps;
  - tool steps run agent-written code unsandboxed.
- `data/orchester.pen`: the person's own change.

Commit these only when asked.

`tools/check.ps1` now formats per package; this came in with ORC-21. ORC-22, ORC-2, ORC-3 and ORC-9 changed the same
file their own way, so each conflicts in `tools/check.ps1` when merged. Resolve by taking master's version. ORC-22 also
conflicts in `docs/plan.md`. ORC-6 and ORC-7 merge cleanly. ORC-6 was told to take master's file exactly.

| Task | Branch / run | State | Next |
|---|---|---|---|
| ORC-21 stop the apps a run started | merged `a7b0520e`, workflow done | done | The installed app lacks the new tools until the person has it reinstalled. |
| ORC-22 Codex ends its turn while a tool call waits | `orchester/orc-22-run-1`, worktree `01a127bf-…` at `516fdf39` | run succeeded; three Claude review rounds; round-3 fixes committed | See ORC-22 below. |
| ORC-2 Agents page | `orchester/orc-2-run-4`, worktree `01a12750-…` | run succeeded, design-verified by the pipeline; independent review: fix first | See ORC-2 below. |
| ORC-3 Pipelines page | run `01a12776-6863-771a-ac12-62e125bd1e06` | running: returned to Implement after Review used its retries | Two major defects: obsolete permission restrictions cannot be removed after a step changes profile; permission errors survive removing the step and block saving. Watch it; after it succeeds, the same review flow. |
| ORC-6 Tools page | run `01a127e1-67d5-71ec-b16a-1e65abb5fadd` | running (Implement) | Told: take master's `check.ps1`, defer the macOS check, finish. |
| ORC-9 Project settings | run `01a127e0-1b9a-74ee-9b9b-c73b424cfeba` | running (Implement) | Told: defer the macOS check (no macOS SDK here), name any `cfg` code, finish. |
| ORC-7 Secrets page | run `01a12804-c440-7009-bbde-6a7ffae52b57` | running (Implement) | The person said to let it run. |
| ORC-23 drive the app a run started | draft, not started | waits for the person | Builds on ORC-21, which is now merged. Start only when the person says so. |
| ORC-4, 5, 8, 10–20 | drafts | not started | Only on the person's word. |

**ORC-22.** The round-3 fixes are:

- a yielded cell's Codex decision holds the continuation;
- a cap of 3 idle continuations;
- a pause while a tool runs checkpoints instead of continuing.

Next: one more independent review of `516fdf39`, the person's OK, then the merge.

**ORC-2.** The review's findings:

1. **Medium.** In `screens/agents/mod.rs:99-116` a rejected reload keeps the old data and shows no error; check
   `is_rejected()`.
2. The "fully enforced" rule is duplicated in `enforcement.rs` and `policy_panel.rs`; move it to workspace.
3. "Only enabled MCP servers" is enforced only in the UI.
4. `Action::KeepDraft` is dead code.
5. Error messages:
   - an unknown MCP server gets the wrong message;
   - `draftChanged` says "publishing" although it also shows on Save;
   - `.map_err(|_| notEnforceable)` drops the detail.
6. The `·` separator literals.

A Claude fixer stopped half-way. Its changes are **uncommitted** in the worktree: workspace `agents/*`, a new
`agents/servers.rs`, `profile_input.rs`, `messages.rs` and `tests/agents/concurrency.rs`. Check whether they are
sound, finish the remaining findings, and run the filtered tests and `check.ps1`. Then:

1. Re-review.
2. Get a design check of the failed-reload state.
3. Verify the confirmations in a QA app under the current confirmation delegation. Actual person-only confirmations
   (including protected stale-draft and widening decisions) still require the person.
4. The person's OK.
5. Merge.

### How a branch gets to master

1. The run succeeds, which means the pipeline's Review, Check and, for UI work, Verify passed.
2. An **independent reviewer** (not the implementer) reviews `git diff master...<branch>`: correctness, `AGENTS.md`
   rules, and security. Be adversarial wherever the change touches permissions, the person-only gate, processes,
   paths or secrets. The pipeline's own Review has missed major defects every time so far.
3. Findings are fixed on the same branch, in the run's worktree, committed `fix(...)`. Then re-review, until the
   verdict is merge.
4. For UI: permitted automated confirmations may be checked by an independent verifier under the current
   confirmation delegation; actual person-only states are checked by the person. Everything visible must pass
   design verification (`.claude/skills/design-verification/SKILL.md`).
5. Ask the person. On OK:
   1. If `docs/plan.md` has local changes, set them aside with
      `git stash push -m <tag> -- docs/plan.md`, apply by SHA and drop the entry by tag.
   2. `git merge --no-ff <branch> -m "Merge branch '<branch>' (<task>: <what>; reviewed)"`.
   3. `cargo check --workspace --all-targets --all-features`. A merge without it once broke master.
   4. `update_task` workflow_status done.
   5. Move the item to `docs/plan.md` Done if it is there.
6. Merged work reaches the person's app only by reinstalling, and only when the person asks.

## Known problems and open proposals

- **The Codex sandbox on Windows** cannot reach an app's MCP discovery file or loopback port. Tests and QA that
  drive the app's MCP (`tools/lurq-mcp-client.py`, `mcp_domain` tests) therefore ask for every call to run outside
  the sandbox. ORC-23 (`call_run_app`) solves it for QA apps. For tests, it is proposed but **not yet approved by the
  person**: let the test harness take the discovery folder from an environment variable inside the worktree.
- **Codex's command approvals.** A Codex step that does not ask before tool calls declines every MCP tool Codex asks
  about; this is in `docs/plan.md` A5. Workaround: Implement asks before tool calls.
- **No macOS checks here.** `cargo check --target aarch64-apple-darwin` cannot build crates with C code on this
  machine, because there is no SDK or C cross-compiler. Agents should list the macOS check as not run and name any
  `cfg(target_os)` code they added.
- **Possibly flaky tests:** `tab_labels`, `summary_run_again…`; noted in `docs/plan.md`.
- **lurq 0.43.0** is released (`run_if_idle`, `outcome()`, `fit_content`, `scroll_into_view`, `is_synthetic_input`,
  `focus_visible`). The bump is plan item A4. Pitfalls of lurq 0.42:
  - futures, streams and `future_action` must sit at stable positions in `render`;
  - a rejected `FutureState` keeps the previous data;
  - `FutureAction::run` replaces an in-flight run.

## Backups

- `.tmp/backup-2026-10-10/` and `.tmp/backup-2026-10-10b/` hold the application and project databases from before the
  project was set up from scratch. They include the `-wal` and `-shm` files and the old `.orchester` folder.
- `.tmp/cleanup/patches/` holds patches of the worktrees that were removed.
