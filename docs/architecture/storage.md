# Orchester — processes and local storage

Date: 2026-09-23. Status: agreed direction for the local (single-user) version; details marked
*open* still need a decision. A future server replaces the local store behind the same
`orchester-core` traits; nothing here is a server contract.

## Decisions

| Topic | Decision |
| --- | --- |
| Project data | `<repo>/.orchester/`, local only, never committed |
| Application data | `orchester.db` next to the executable; a macOS app bundle keeps it in `~/Library/Application Support/Orchester/` (see [Storage layout](#storage-layout)) |
| Structured data | SQLite (WAL) |
| Large content | content-addressed blob files |
| Secrets | OS keychain only; databases store references |
| Execution | separate runner process; agents are runner child processes |
| Agent worktrees | inside the repository, `<repo>/.orchester/worktrees/`; a step running beside other steps of its run has its own, merged back when it passes; a run's worktrees go when it is merged, or when a person removes them after it ended (see [Integration](#integration)) |
| Integration | a run that succeeded is merged into the branch the user's checkout had when it started, in that checkout, only while it is clean and on that branch; nothing of the user's is stashed, reset or discarded |
| Environment | a project adds plain variables (no secrets) to the cleaned environment of every agent CLI and command tool its runs start, passed as written |
| Closing with active runs | confirmation listing the runs: wait for checkpoints, or quit now |
| Quit now or crash | next start reports the abnormal end and offers to continue from the last checkpoint |
| Logs and transcripts | kept in full, no retention limit |
| Pull request diffs | cached per revision |
| Run diffs | read with git from the run's base to its latest checkpoint, cached per pair of commits; changes not checkpointed are read from the worktree each time |

## Processes

```
orchester.exe (desktop, UI)
 └─ orchester-runner.exe  (one per open project)
     └─ agent CLIs (Codex, Claude, …), one per active step, each in its own worktree
```

- The desktop owns the window and all user-authored changes. It starts one runner
  per project a window tab shows, keeps it while the project has work (a run in progress or
  waiting for an agent slot, a checkpoint being saved) after its last tab closed, and then stops it
  with `StopIfIdle`, which the runner refuses while it has work. It talks to it over a local IPC channel (a named pipe on Windows,
  a Unix domain socket in the user's private `$TMPDIR/orchester/` folder on macOS; the desktop
  checks that the process serving it is the runner it started): commands down (start, pause, resume, cancel, deliver message, approval decision), events up
  (state changes, progress, output). It reads state directly from the project database.
- The runner owns execution: scheduling, agent processes, checkpoints, tool calls, evidence. It
  resolves secrets from the keychain at tool-call time; neither the desktop nor agents see values.
- Runner and agents run inside a Windows Job Object created by the desktop with
  kill-on-close, so they can never outlive the desktop process, whether it quits or crashes.
- macOS has no Job Objects. The desktop sends the runner SIGTERM when it lets go of it (SIGKILL
  if it is still running 30 s later), and the runner ends its work when the desktop's end of the
  connection closes, which is also what a desktop crash does; either way it ends its agents first.
  Each agent CLI and command runs as the leader of its own process group; the runner ends it with
  the group (SIGTERM, SIGKILL after 3 s), and ends the same way whatever it left running when it
  exited. Unlike a Job Object, a process that leaves its group on purpose (`setsid`) survives.

### Lifecycle

Closing the window is closing the application; there is no background mode. (A tray icon that
keeps work running is a possible later improvement, not part of this version.)

- **Close with no active runs** → the application exits cleanly.
- **Close with active runs** → the close request is vetoed and a modal lists the tasks in
  progress (task, current step, agent). It offers:
  - **Wait for agents to save** — the runner stops scheduling, lets every active step reach its
    next checkpoint, marks the runs paused and exits; then the application closes. The modal
    shows per-task progress while waiting and still offers Quit now.
  - **Quit now** — runner and agents are terminated immediately, without a checkpoint.
  - **Cancel** — keep the application open; nothing changes.
- **Start** → the desktop reads the previous session:
  - after *Wait for agents to save*, the runs are simply paused and can be resumed as usual;
  - after *Quit now* or a crash (power loss, process killed), it reports that the session ended
    abnormally, lists the interrupted runs with the reason, the last persisted checkpoint of
    each and whether its worktree has uncommitted changes, and offers to continue each run, or
    all of them, from its last checkpoint. Nothing restarts automatically: the modal offers
    only continuing all of them or none. Runs not continued there stay interrupted and each can
    be continued on its own later from its task conversation (user decision, 2026-09-24). Work after the
    checkpoint is not claimed as done; uncommitted worktree changes are kept and shown, never
    discarded silently. A run that waited for a person's answer or plan decision, with no agent
    or step at work, was not interrupted: its agent's session resumes with the answer, so it is
    not listed and keeps waiting. One that waited on a tool or command approval was: the agent
    process held the request.
- **Runner drops while the application is open** → the desktop reconnects automatically with a
  bounded backoff and tells the user (reconnecting, reconnected, or could not restart). Runs
  interrupted by the drop continue automatically from their last checkpoint as soon as the runner
  is back; nothing after the checkpoint is claimed as done. If the runner cannot be restarted, the
  runs stay interrupted until it is.

lurq already supports vetoable close requests whose decision can be held for an asynchronous
dialog (`on_close_requested` / `CloseRequest`), which is what the wait path needs.

### Step graphs

The runner starts every step execution whose dependencies have passed, as far as the agent slots
allow; the slots (the project setting *Agents working at once* in Project settings › Limits, 1 to 16, 4 by
default) are shared by all runs of the runner, and each agent at work takes one. The desktop sends them when
the runner attaches and again whenever they change (`SetAgentSlots`): more slots start waiting work at once;
fewer stop no agent, and new work waits until fewer agents work than the slots.

- **Where a step works.** A step that starts alone — no other step of the run is running or ready —
  works in the run's worktree on the run's branch, as a sequential pipeline always has; nothing else
  can start while it runs, since only a passing step makes another one ready. A step that starts
  beside others, read-only or writing, gets a worktree of its own on `<run-branch>--<step>-<attempt>`,
  created from the run's branch as it is then (it holds every step it depends on). Parallel writes
  in one worktree would overwrite each other, and a merge would change files under an agent at work.
- **Joining.** When such a step passes, its work is committed on its branch and merged into the
  run's branch in the run's worktree, one merge at a time, with the repository's git identity (fast
  forward when possible, else a merge commit); its worktree and branch are then removed. A conflict
  is undone, the step fails with *merge conflict* (the files are named in the transcript) and restarts
  by itself, as any breakage does (below). Its branch and worktree stay.
- **Failures.** A step fails when its agent fails, when its structured result says `failed`, when it
  cannot start or when its work cannot be committed or merged. A failure ends its attempt: agents of
  the attempt still at work are ended and their executions skipped. What a failed step changed is
  committed as a checkpoint of its execution, never discarded. Failures for a used step budget,
  recorded by earlier versions, are still read. What follows depends on the failure (user decisions
  2026-10-02, `orchester_core::run::Run::fail_step`):
  - A **verdict** on the work — the agent reported `failed` (also when its turn failed after it
    delivered that result, e.g. a permission prompt that closed afterwards), or a command tool's
    program exited with a code the tool does not expect or timed out — follows the step's failure
    route: a new attempt from the route's target and everything after it. Without a route the run
    fails. Once the route was taken `max_retries` times the run **waits for a person**.
  - Any other failure is a **breakage**: the agent CLI failed its turn without a `failed` result (it
    crashed, its turn failed, its permission prompt closed or never reached the runner — a `done`
    result never outweighs that), or the step could not start, commit or merge. The step restarts
    as a fresh execution in a new attempt (reason *restart*: that step, its dependent and its
    unfinished steps; never its route's target). A step that broke twice since it last passed —
    counted across attempts, so steps that break in turn do not restart each other forever — makes
    the run wait for a person. A follow-up that fails is not routed at all.
  - At most 10 automatic attempts (retries and restarts) follow each other; any failure after them
    makes the run wait for a person.
  - A run **waiting for a person** (*Needs you*) holds no agent slot. The person allows one more
    retry (the same route, or for a broken step the same restart), returns the work to an agent
    step the failed step runs after (or the failed step itself) with a note — an attempt with reason
    *returned* whose note the step's prompt carries as the person's instruction — or stops the run,
    which fails it as before (retries exhausted, or step failed for a breakage). A run that waits can
    also be cancelled. Messages written meanwhile stay queued until it continues.
  - The **prompt of a retry attempt** tells the route's target, or the step a person returned the
    work to, that it came back from the failed step in attempt *n*, with that step's whole result
    (its summary and handoff lists) or, for a command tool, how it ended and the end of its output;
    the failed step itself gets its own earlier result to check each finding. A restart of either
    step carries the same report and the person's note. The report is quoted
    as untrusted output and kept within 12 KiB (an agent's from its start, a program's output from
    its end, each with a cut marker).
- **Step budgets.** A step whose execution uses its budget does not fail: its agent is ended after
  its turn, its work is committed as a checkpoint in its worktree, and the run stops at its limit
  (*Stopped at limit*, naming the execution). Agents of other steps finish their invocations, as at a
  pause, and nothing new starts; no retry is taken. Raising that step's budget for the run (above
  what the execution used, with no maximum; empty lifts it) continues the stopped agent's CLI
  session. The raised budget applies to this run only; the published pipeline keeps its own.
- **Outputs.** A new agent session ends with a structured result (Codex `--output-schema`,
  `{outcome: done|failed|question, summary}`); its summary — or, without one, the final message — is the
  step's output, and the prompts of the steps that depend on it include it with its source (step,
  attempt). A continued session is started without the schema (the probe verified it for new
  sessions only), so its CLI exit and final message decide. Claude Code runs without a schema
  (`--json-schema` could not be verified): the prompt asks for the same result as the final
  message, and when it is not one the CLI's `is_error` and exit decide.
- **Approval gates.** A step with an approval gate that passes proposes its output (the summary of its
  structured result, else its final message; its prompt asks for the complete result) as the next
  plan of that step in the run, and waits for a person (plan review, 02.11). While nothing else of the
  run works or can start, the run's task ends: it holds no agent slot, and waiting is not active agent
  time. *Approve* passes the step; the steps after it read the approved plan as its output. *Request
  changes* carries a note: the step's next invocation resumes its CLI session told what should change
  (a new session, when the CLI reported none, gets its earlier plan and the note), and its result is the
  next version, which needs a decision of its own. A run that only waits keeps waiting across a new
  runner or session (it is not interrupted); a pause of it is immediate, and a decision needs it running.
- **Sessions.** Codex reports its session (the thread of `thread/start`); a Claude Code session gets an
  id Orchester chose (`--session-id`), which the CLI reports back (`system/init`). Continuing a step
  resumes the session its last invocation reported (`thread/resume`, `claude --resume`).
- **Task conversation (M8).** A person's message goes to Orchestrator, which routes it to the agent
  of the task's run whose invocation starts next, or to the agents of one profile; it is written for
  the task's latest run (for its next run when it has none). The runner delivers messages at the
  run's next invocation boundary: every agent invocation that starts takes the messages queued for its
  agent into its prompt (a new session in its task part, a resumed one first, told why it reads them
  now), and they are *delivered*. An agent that finishes its turn — its CLI exits on its own — while
  messages for it are queued does not end its step: its work is committed as a checkpoint and its
  session resumes to read them, as with *Apply now* without a change of permissions. The turn that
  ends after a delivery *acknowledges* the messages, and its final message (its summary, when it is
  a structured result) is the agent's reply in the conversation. A paused, stopped or interrupted
  run keeps messages queued until it continues; a failed run, or one that waits for a person's decision,
  needs an explicit recovery message (below), while a cancelled run takes none. The desktop never leaves a
  message without a reader (ORC-79): its composer resumes a paused run (`ResumeRun`) or continues an
  interrupted one (`ContinueRuns`) right after storing the message, keeps the message of a run stopped at
  its own limit until the person raises it, and for a run whose agents read no more messages stores it with
  no run (the next run's first invocation takes it) and runs the task again (`RunAgain`, the message's
  idempotency key as the request's).
  *DeliverMessages* only wakes a run; what is queued is read from the database.
- **Send now (ORC-61).** A person's message is sent after the turn (`timing = 'after_turn'`, as above)
  or now (`'now'`). The runner gives each message sent now and still queued, once, to the first agent at
  work it reaches, while its turn runs: Codex with App Server's `turn/steer` (`expectedTurnId` the running
  turn), Claude Code as another stream-json user message on its standard input (Claude Code now runs
  with `--input-format stream-json --replay-user-messages`; the prompt is its first input line, and the
  input closes once a turn ends). Both take it after the tool call that is running (checked live
  2026-10-02, `docs/evidence/orc-61-send-now-2026-10-02.md`). It is *delivered* when the CLI shows it
  took it — Codex's `userMessage` item, Claude Code printing it back — with `arrival = 'mid_turn'`, or
  `'turn_end'` when Claude Code ended its turn first and takes it as its next turn. A message the turn
  ended before (Codex: *no active turn to steer*; Claude Code: its input already closed) stays queued and
  is delivered when the session continues to read it (`arrival = 'turn_end'`, as for every message the
  continued session takes; `'prompt'` for one a new invocation started with). One the CLI refuses
  (`undelivered_reason = 'refused'`) or that cannot be written to it within five seconds
  (`'unwritable'`) is *not delivered*: it is kept, goes no further, and is queued again only when the
  person chooses *Send after this turn* (the desktop's write; `timing` then becomes `'after_turn'`). If
  the CLI takes one late after all, it is delivered then. Codex keeps a message it accepted (`turn/steer`
  answered with the turn): it gives it to the model at its next sampling or, when the turn finishes
  first, records it into the thread before `turn/completed` (Codex's `on_task_finished`). Such a message
  is delivered at that `turn/completed` (`arrival = 'turn_end'`) unless the turn was interrupted, so it
  is never queued for a next turn as well; an interrupted turn, or a process that ends without
  `turn/completed`, leaves it queued. An acceptance read after `turn/completed` (Codex may answer the
  steer late) delivers it at the turn's end too, since Codex accepts only while the turn runs. A steer
  still unanswered when the process ends stays queued and goes to the next turn: Codex may then read
  it twice, which is chosen over a message shown as delivered that Codex never had. While a message
  is on its way to one agent at work, no other agent's prompt takes it (parallel steps); it is queued
  for any agent again once that agent ends without taking it. The desktop's *Send after this turn*
  and the runner's late delivery change a message only while it still stands as each read it
  (`change_delivery`): the first wins, the other is told and leaves it (the desktop says it moved
  on), never failing the step. Known limit: Claude Code's printed-back message is matched to what
  was written by its exact text; if Claude Code ever printed several messages back as one, none
  would match and they would stay queued and be given again at the turn's end.
- **Questions.** A result with the outcome `question` asks the person before the step can go on: its
  work is committed, the question is the agent's message, and the step waits for a message (like a
  plan, a run that only waits holds no agent slot and no active time). A message that reaches it
  resumes the session with the question and the answer. Claude Code's final message is read the same
  way as for done and failed; only a final message that is exactly the JSON result counts, so prose
  is never taken for a question.
- **Follow-ups (Ask agent).** A person's message to a run that succeeded reopens it: an attempt of
  its own (reason `follow_up`, naming the step) executes the last agent step in pipeline order — of
  the recipient's profile, for a direct message — once more, continuing the CLI session of that
  step's passed execution in the run's worktree, so its work makes new checkpoints. The run is
  running meanwhile and succeeded again when the step passes; the run's limits apply as to any work.
  Its reply is its message in the conversation, not a new output. A follow-up that fails leaves the
  run succeeded with the result it had; its attempt stays recorded as failed and decides nothing. A
  run that succeeds while messages are still queued for its agents (e.g. written while a tool step
  ran) is followed up at once. A run whose work was merged into its base branch takes no messages. A
  question answered while no runner was attached resumes its step once a runner attaches, and a run
  that succeeded, was not merged and was written to meanwhile is followed up then, once; messages to
  a merged or cancelled run stay queued; a failed run resumes only with an explicit recovery request. A follow-up needs the run's worktree: once a person
  removed it, the run is not followed up (the runner refuses with *worktree removed* before changing
  it) and messages to it stay queued.
- **Failed-run recovery (Send & continue).** The Conversation composer stays open for the latest failed run
  when its worktree survives and an executed agent can receive the instructions. A message explicitly carries
  the failed run's event sequence visible when the person sends it. The runner accepts that marker only from
  the latest `Failed` event through the current last sequence: old queued messages, stale composers from an
  earlier failure, and duplicate notifications cannot create repeated automatic attempts. This works across
  runner reconnects and equal message/failure timestamps. An old run cannot resume after the task started another.
  A run that waits for a person after its retries ran out or its step broke again is continued the same way
  (ORC-79): the marker counts from its latest `WaitingForPerson` event, the failed step is the one it waits on,
  and a `RetryAllowed` event (the person let it go on) takes the place of `RecoveryRequested`; the attempt it
  kept open is routed back.
  A `RecoveryRequested` event and an attempt with reason `recovery` retain the same run ID, branch, worktree,
  checkpoints and previous attempts. Orchestrator routes to the failed agent, or its latest executed upstream
  agent if a command failed; a direct recipient can return to its own executed agent step. That step, its
  downstream steps and unfinished steps execute again; other passed steps remain passed. Existing provider
  sessions and retained parallel worktrees are reused, looking past attempts that never started the step.
  A recovered parallel worktree is removed by its original owner ID only after its branch has merged. Recovery
  prompts preserve partial/uncommitted work and include the earlier result, new instructions and current
  dependency outputs. Missing provider sessions start a new session in the retained workspace. Worktree removal
  blocks recovery without recreating it; missing agents are visibly unavailable. Recovery respects the run's
  saved permissions, budgets and pinned configuration. A subsequent error is recorded as another failed attempt;
  the person can send fresh instructions again. It does not classify arbitrary prose as a question or failure.
- **Tool steps (M10a).** A tool step runs a published command tool: its program with fixed
  arguments, no shell, found on the `PATH` or in the worktree and started like every program
  (command scripts, the cleaned environment plus the tool's variables, no console window, its
  process tree ended with it on Windows). It works where an agent step would: alone in the run's
  worktree, beside others in a worktree of its own merged back when it passes. It takes no agent
  slot — a command is no agent and uses no tokens — but one of two command slots the runner's runs
  share, since a check such as a test suite can use every core. Its run is an invocation of kind
  `command`, so its time counts toward the run's time limit; the limit, a cancel or the runner
  stopping end it, and a run limit reached by an agent's turn lets it run to its end, as a pause
  does. Its output (both streams, in order) is kept up to the tool's limit and recorded in the
  transcript when it ends, with how many bytes before it were not kept. An exit code the tool
  expects passes the step; another code or the tool's timeout fails it (*command failed*) for its
  failure route. A short summary (the command, how it ended, its last lines) is its output for the
  steps after it. HTTP and database tools, which use secrets and approvals, and command tools with
  access rules are refused by the preflight.

### Integration

A run that succeeded is merged by the desktop, in the user's checkout (the repository folder), into
the branch that checkout had when the run started: the runner records it with the run (`runs.base_branch`;
none on a detached HEAD, and none for runs recorded before it was kept, which cannot be merged from
Orchester). All of it is git through `orchester-exec`, with git's messages in English and no optional
locks (`orchester_core::integration`).

- **Only a checkout that is ready.** The checkout must be on that branch and have no merge,
  cherry-pick or revert in progress; otherwise the merge is refused with the reason and nothing
  changes. Uncommitted changes and untracked files may be there: nothing is ever stashed, reset or
  discarded, and what the merge does not reach stays as it is.
- **Merging.** A fast-forward when the branch did not move since the run started, else a merge commit
  *Merge ORC-12 run #3* with the repository's git identity, made in a temporary worktree
  (`<git common dir>/orchester-merge-<run id>`, removed again on every path; a folder git refuses to
  remove or no longer knows goes as a folder, and one that still stays is reported beside the merge,
  which stands) so that the merge never touches the checkout's files. A conflict, or a hook that
  refuses the merge commit, ends with the temporary worktree removed, the checkout as it was, and the
  conflicting files named. If the checkout moved to another branch, or its branch got a commit,
  while the merge commit was made, the merge is dropped and refused.
- **Hooks.** The repository's own hooks run: they are the user's. The merge commit's hooks
  (`pre-merge-commit`, `prepare-commit-msg`, `commit-msg`, `post-merge`) come from the hooks folder
  the checkout uses (`core.hooksPath`, a relative one resolved against the checkout) but run with the
  temporary worktree as their working folder, which holds the tracked files only: nothing ignored,
  untracked or uncommitted of the checkout's, so no installed tools. `post-merge` runs there and once
  more in the checkout when the merge is brought in.
- **Bringing it in.** Either result reaches the checkout as a fast-forward (`git merge --ff-only
  --no-autostash`), which git refuses, changing nothing, when it would overwrite or remove a file
  with uncommitted changes, an untracked file or an ignored one; the refusal names exactly those
  files (or the folder that holds untracked files). `merge.autoStash` never stashes the person's
  work. Should git still stop part way, e.g. at a file another program holds open, the files it
  wrote are put back; any it could not put back are named. A branch that already holds every
  commit of the run is recorded as merged without a change.
- **Rebased runs.** A run keeps the base it started from, so after its branch was rebased onto
  newer commits of the branch it merges into, the commits between the two would read as the run's
  own work already integrated. When git proves the rebase, the base its own commits sit on now is
  recorded (`run_rebases`, one row per run, a newer rebase replacing it) and external evidence reads
  the run from there: the branch's reflog names the commit the newest rebase put it onto, that commit
  descends from the recorded base and is below both the branch tip and the target, and no checkpoint
  of the run lies between the recorded base and it (else the target has work of the run's own). A
  last checkpoint the rebase rewrote is read as the branch tip when their commits are the same
  patches, whitespace included (`git patch-id --verbatim`). Without a rebase, a run whose own
  commits the target has, by ancestry, a pick or the same tree, still shows as integrated. After a
  proven rebase, only the run's checkpointed commits the target has by ancestry stop the base from
  moving: a commit of the run's the target has as a pick, or one never checkpointed, below the
  commit the branch was rebased onto, is not told apart from the target's own (the rebase itself
  leaves out of the branch a commit whose patch the target already has).
- **Runs that took the base in (ORC-60).** A branch fast-forwarded onto a newer commit of its target (an agent
  told to merge the base before it had committed anything), or one that merged it, holds the target's commits
  above the recorded base. Their newest one is the base its own work starts from
  (`orchester_core::integration::fork_point`): one a `…: Fast-forward` entry of the branch's reflog moved it to, a
  base recorded before, or the newest commit branch and target share, as long as no commit of the run's own is at
  or below it. The run's own commits are its checkpoints and the commits its branch's reflog shows were made on it
  (commits, merge commits, picks, rebases; not fast-forwards or resets). External evidence and Merge into main read
  the run from there, and so do Changes and *What changed*; the run's recorded base stays its start for history.
  Writing agents commit a checkpoint before they merge the base, and merge it with `--no-ff`.
- **Recording.** The merge is a desktop-written row (`run_merges`: the branch, the commit it was at
  afterwards, fast-forward / merge commit / already merged, when). The task is then *Merged*.
- **Clearing up.** Right after a merge the run's worktrees (its own and those of failed or skipped
  steps that ran beside others) are removed with `git worktree remove`, build output included, and
  its branches whose commits are on the base branch or the checkout's current branch are deleted;
  other branches are kept, and so is a worktree with uncommitted changes. For a run with no live
  work — one that ended (succeeded, failed or cancelled) or was interrupted — a person can remove
  them too (*Remove worktree*, from the top bar for the latest run and from *Runs of this task* for
  any run, ORC-63): a worktree with
  uncommitted changes goes only once the person saw the files named and agreed to discard them, and
  only if the changes are still exactly those. Run again can remove them the same way, in the
  runner, before the task's next run starts. A folder already deleted by hand is pruned, and a folder
  git forgot after a removal that failed part way is deleted as a folder. Such a folder's files are
  listed like uncommitted changes, but only its first 200 entries in name order are read (past them
  it holds more), and an entry that cannot be read ends its listing with why, a problem of that run's
  row rather than of the whole read; it goes only once the person agreed to it as read. A removal is recorded
  (`run_worktree_removals`) before git runs, in the write transaction that re-reads the run and
  refuses it once the run has live work; recording a continuation of a run whose removal is recorded
  is refused in its own write transaction, so the two never both go ahead. The record is taken back
  when nothing was touched and stays once anything went or git refused. An interrupted run whose
  removal is recorded (or whose folder is found gone) stays interrupted in its record, but it never
  continues and no longer keeps its task from running again. The
  Changes tab then shows the checkpointed changes and says the worktree was removed; a run's commits
  stay reachable through the base branch or its kept branch.

## Storage layout

```
<install dir>/
  orchester.exe
  orchester-runner.exe
  orchester.db                 application database
  logs/orchester-<YYYY-MM-DD>.log  desktop log (UTC date), the last 14 days kept

<repo>/.orchester/             excluded through .git/info/exclude (the project's .gitignore is not touched)
  orchester.db                 project database
  blobs/<aa>/<sha256>          zstd-compressed content
  worktrees/<run-id>/          agent git worktrees: the run's, on its branch
  worktrees/<execution-id>/    a step running beside others, on `<run-branch>--<step>-<attempt>`
  skills/<execution-id>/<slug>/ the published skills that reach a step execution's agent (SKILL.md and
                               reference files), written again at each start of the execution and kept
  step-result.schema.json      the JSON schema new agent sessions end with (Codex `--output-schema`)
  logs/runner-<YYYY-MM-DD>.log runner process log (UTC date), kept in full
  runner.lock                  one runner per project
```

A macOS app bundle cannot write beside its executable (`Orchester.app/Contents/MacOS/` is part of
the signed bundle, which an update replaces), so a bundled build keeps its data in the user's
Library; the runner stays beside the desktop in `Contents/MacOS/`:

```
~/Library/Application Support/Orchester/orchester.db   application database
~/Library/Logs/Orchester/orchester-<YYYY-MM-DD>.log     desktop log (Console.app, Log Reports)
```

A macOS build outside a bundle (a development build in `target/`) keeps both beside its executable,
as on Windows, so it never opens the installed app's database. The decision is made in one place,
`crates/desktop/src/app_paths/`.

*Open:* a directory next to the executable is not writable under `Program Files` and may be
replaced by an installer. The path is resolved in one place so a per-user fallback can be added
when a Windows installer exists.

## Conventions

- **Identity:** UUIDv7 for every row id (time-ordered, merge-safe for a future server). Display
  keys (`ORC-129`, run `#128`) come from per-project counters and are never used as references.
- **Time:** UTC, milliseconds since the Unix epoch.
- **Budgets:** whole numbers of tokens (input + output as the CLIs report it; cached input is not
  counted), step budgets included. Published pipeline versions from before step budgets were tokens
  keep their money amounts in their bodies (they are immutable); such a step reads as having no step
  budget, and the amount is kept in `money_step_budgets_before_tokens`.
- **Time limits:** milliseconds of active agent time (the time agent CLIs ran); pauses, stops at a
  limit and waits for a person do not count.
- **Revisions:** full 40-character commit SHA.
- **Structured bodies:** JSON columns with a `schema` field for resource bodies, so a body can
  evolve without a table migration.
- **Idempotency:** every user action that creates something (task batch, queueing, message,
  feedback, run start) carries a client-generated key with a unique constraint; a retry returns
  the existing result.
- **Append-only:** run events, messages, audit events and published versions are never updated
  or deleted. Current state is stored alongside for fast reads and must be derivable from them.
- **Migrations:** each database has a `meta.schema_version`; migrations are forward-only and run
  at open. A newer schema than the binary understands refuses to open instead of guessing.

## Application database

| Table | Purpose |
| --- | --- |
| `meta` | schema version |
| `projects` | id, name, repository path, added / last opened |
| `settings` | application settings (locale, runner limits) as key → JSON |
| `sessions` | id, pid, started / ended, `end_kind`: clean, runs_paused, quit_now; still open means crash |
| `ui_state` | key → text: the window's tabs (`tabs`: their locations in order and the active one, without history); later window placement and drafts not tied to a project |
| `servers` | the shared servers (`server.md`, "Desktop data"): id, normalized address (unique), the server's name, plain HTTP allowed, added at, and the account signed in from this computer (user id, username, display name, admin, signed in at). The token is in the OS keychain under `orchester-server:<server id>` / `token` |

A session row is written at start with `ended_at = NULL`. A clean exit fills it; a row still
open at the next start means the previous session crashed.

## Project database

### Project

| Table | Purpose |
| --- | --- |
| `meta` | schema version, project id |
| `project` | key (`ORC`), name, repository, default branch |
| `counters` | next task number, next run number |
| `project_limits` | at most one row: default run budget and default time limit (each `NULL` for no limit), daily project budget, agent slots; without it the defaults apply (no run budget, 20M tokens a day, no time limit, 4 agents). There is no maximum run budget (user decision 2026-10-09) |
| `project_environment` | Project settings › Environment: variable name (unique without regard to case) and plain value, added to the cleaned environment of every agent CLI and command tool the project's runs start, under a command tool's own variables; read at each start |
| `money_budgets_before_tokens` | the money budgets tasks and runs had before budgets became tokens, kept by the migration that replaced them with the default run budget |
| `money_step_budgets_before_tokens` | the money step budgets of published pipeline versions and drafts from before step budgets became tokens, per version or draft and step |
| `repository_link` | at most one row: GitHub connection, git remote, host / owner / name, default branch, permission, expected login, checked at |

### Versioned configuration

Agent profiles, instructions, pipelines and tools share one lifecycle: one editable draft,
immutable published versions `v1..vN`. Runs pin versions; publishing affects new runs only.

| Table | Purpose |
| --- | --- |
| `resources` | id, kind (`agent_profile`, `instruction`, `pipeline`, `tool`), slug, name, archived |
| `resource_drafts` | resource id, body, based-on version, updated at |
| `resource_versions` | id, resource id, number, body, content hash, published at; unique (resource, number) |
| `resource_imports` | a resource copied from another project (Import resources): import key, source project (id and name), source resource, source version (none for a draft), imported at; unique (import key, source resource). For display only: the copy does not follow its source |
| `connections` | kind (GitHub, agent provider, MCP), name, settings, keychain reference, status, last check |
| `secrets` | name, kind, environment (development, staging, production), created / rotated / archived; unique (name, environment) among secrets not archived. No column holds the value: the keychain entry is named by the project and secret ids (Windows Credential Manager: generic credential `orchester:<project id>/<secret id>`, local to the computer; macOS login keychain: service `orchester:<project id>`, account `<secret id>`). Archiving deletes the value and keeps the row |

Tool access policy (allowed pipelines and steps, environment, approval mode, access window,
denied behavior) is part of the tool version body. HTTP and database tools must have one; a command
tool (body schema 2: program, arguments, timeout, expected exit codes, environment additions and
output limit, no secrets) has none, and any tool step of a project pipeline that names it may run it.
Bodies of schema 1 are upgraded when read: a command gets the defaults (10 minutes, exit code 0, no
variables, 256 KiB). "Used by" is derived by query, never stored.

Agent permissions (repository access, commands, network) are part of the agent profile version body
since body schema 2. Bodies of schema 1 are upgraded when read, never rewritten: they get read-only
access, commands confined to the worktree and network off (`AgentPermissions::before_permissions`).
A pipeline step body may carry a `permissions` narrowing (commands, network); steps without one run
with their profile's permissions. A run can change one profile's permissions for itself (02.J): from the next invocation of each of
the profile's steps, or now, when the profile's agents at work finish the command they run, their work
is committed as a checkpoint and each step continues as a new invocation that resumes its CLI session.
The connection's CLI must enforce the change for every step of the profile; the published profile
keeps its permissions unless the change is also saved to its draft.

Import resources copies resources from another project the application database knows. The
desktop opens that project's database read-only (SQLite refuses writes; nothing is migrated, so a
database this build has not migrated yet is refused until its project is opened) and writes every
copy into this project in one transaction: a new resource whose only content is a draft, with its
`resource_imports` row. What a copy refers to is resolved first: another resource is copied too or
replaced by one of this project's; a connection or secret is never copied, only mapped to one of
this project's or left unresolved, and an unresolved one stays named in the copy, which then shows
it as missing. A tool copy keeps allowed callers only for pipelines copied with it.

### Tasks and scheduling

| Table | Purpose |
| --- | --- |
| `tasks` | key, title, brief, acceptance criteria, pipeline resource, limits (token budget and time limit, each `NULL` for no limit), status (`draft` or `queued`: submitted, which a finished run does not change; see "State machines"), idempotency key |
| `task_creation_snapshots` | task, title, brief, criteria, pipeline (local id or shared slug), limits — what a task asked for when it was created, immutable (triggers refuse update and delete); a task created before the table has no row, never reconstructed (ORC-25) |
| `task_dependencies` | task, prerequisite, condition (`ready_for_review`, `merged`) |
| `run_groups` | mode (parallel with limit / sequential with gate), budget cap, failure policy, scheduling paused, pinned versions |
| `run_group_items` | group, task, position |
| `project_sources` | position, path — the project's sources (M12): absolute paths of folders and files outside the worktree its agents may read without asking; this machine's, never shared |
| `task_sources` | task, position, path — a task's sources, as `project_sources`; a pipeline's and its steps' are in its versions, and a run keeps the union it started with in its version snapshot |

### Execution (written by the runner)

| Table | Purpose |
| --- | --- |
| `runs` | task, number, group, version snapshot, status and reason (a pause is requested or at a limit; at a step's budget with the execution that used it; waiting for a person, with the step whose retries ran out or that broke again), branch, base revision and the branch the user's checkout had there, worktree, token budget and time limit at creation and in force (`NULL` for no limit), tokens used, pause requested |
| `run_merges` | a run that succeeded and was merged into its base branch: the branch, the commit it was at afterwards, how (fast-forward, merge commit, already merged), when — desktop-written, once per run |
| `run_worktree_removals` | when a run's own worktree was removed (*Remove worktree*, a merge, Run again), once per run — written by the desktop or the runner, whichever removed it |
| `run_step_budgets` | step budgets raised for one run after it stopped at them, per run and step; `NULL` lifted the step's budget |
| `run_permissions` | permissions changed for one run, per run and agent profile: the permissions the run's steps on that profile use instead of the profile's (each step still narrows them), the agents to restart with them at their next checkpoint (*Apply now*) and the last one that did |
| `attempts` | run, number, status, reason (initial, retry, a follow-up naming the step it asks, recovery, a restart naming the step that broke, or returned by a person to a step with their note) |
| `step_executions` | attempt, pipeline step, agent profile version, effective model, status, failure reason (agent failed, reported failed, not started, merge conflict, checkpoint failed, command failed; step budget only in records of earlier versions) |
| `step_progress` | step execution, text, reported at — the one line an agent last reported on what its step is doing (`report_progress`); the latest replaces the one before |
| `step_outputs` | step execution, run, step, outcome (done, failed, or none when the final message was not a structured result), text — what the step left for the steps after it, read into their prompts with its source; append-only |
| `plans` | run, step, version (per run and step, from 1), step execution, text, created; decision (none while proposed, approved, changes requested with the reviewer's note), who decided (the person or automation through Orchester's MCP server; decisions recorded before it was kept count as the person's) and when — written with the run events that propose or decide it; the text never changes and a plan is decided once |
| `checkpoints` | run, step execution, revision, agent state blob |
| `run_events` | run, sequence, time, kind, payload — append-only |
| `evidence` | run, revision, kind (criteria, tests, review, browser), result, blob |
| `artifacts` | run, name, blob |
| `invocations` | run, step execution, number, kind (agent, command), model (empty for a command), CLI session id, started / ended, input / cached input / output tokens, outcome — one row per agent CLI launch or command tool run |
| `agent_events` | run, sequence, step execution, invocation, time, kind, payload — the agent transcript, append-only. Payloads hold what the agent wrote and ran, including file content: an agent CLI's call of Orchester's `edit_file` keeps the edits' old and new text, and `write_file` up to the first 200 lines of the content it wrote (each line up to 1,000 characters). Only the values of the project's registered secrets are redacted, as whole texts before the lines are cut; anything else, such as an unregistered `.env` file the agent writes, is stored as written |
| `invocation_prompts` | invocation, injection event (start for a new session, resume for a continued one) and the source of each part of its prompt in order (profile version, instruction versions, pipeline step, task) — written once before the CLI starts |

### Memory

Task and project memory (`docs/design/orchester-workspace.md`, "Task and project memory").

| Table | Purpose |
| --- | --- |
| `memories` | one record: id, task (none: project memory), kind, optional key (unique among the active records of its scope), text, tags, importance (low, normal, high), confidence (low, medium, high), status (proposed, active, superseded, deprecated, rejected, withdrawn), pinned, the record it supersedes, author (agent with profile, run, step and execution, or person), source transcript entries, created / updated / edited by a person, revision (an update names the revision it read; a changed record refuses a stale write); `seq` is the stable rowid the indexes use; scope and origin never change; deleting a record deletes the records it really superseded, not the one a proposal would replace |
| `memories_words` | FTS5 index (unicode61 tokenizer) of the records' text, key and tags, for BM25-ranked word and prefix search; external content, kept in step by triggers |
| `memories_trigrams` | FTS5 index (trigram tokenizer) of the same columns, for substring search: other word forms, file paths, identifiers |
| `memory_settings` | at most one row: how agents' writes to task and to project memory are kept (automatic, after approval) and the injection limits (1–50 records, 500–20,000 tokens); without it the defaults apply (task automatic, project after approval, 8 records, 3,000 tokens) |
| `invocation_memories` | invocation, position, record id, why it was chosen (handoff, pinned, decision, warning, match), its estimated tokens and what became of it (given, truncated, over the record limit, over the token limit) — what the invocation's prompt was given in prompt order, then what the limits left out; written once before the CLI starts; a record deleted later keeps its id here |

### Conversations and pull requests

| Table | Purpose |
| --- | --- |
| `conversations` | *not built yet*: project and pull request thread conversations (scope, title); a task's conversation is its messages |
| `messages` | task, run, idempotency key (a person's), author (person, agent), kind (an agent's reply or question), recipient profile (a person's; none: Orchestrator), step execution and invocation (delivered to, or written by), body, created, delivered, acknowledged, optional immutable recovery event sequence — what a message says never changes, it is never deleted and its delivery only moves forward; attachments are not built yet |
| `questions` | one request of an agent that asks the person questions with suggested answers (ORC-83): run, step execution, invocation, agent role, the questions as JSON (text, optional header, at least two options with optional descriptions, whether several may be picked), whether the agent's turn waits (`in_turn`) or its step ended with it (`step_end`), asked at, state (waiting, answered, interrupted), the answers (labels picked and own text, checked against the questions when read), who answered (person, automation), ended at, and for a step-end answer the person's message it reached the agent in; written by the runner, answered once |
| `proposals` | conversation, version, proposed task set (not tasks until created) |
| `pull_requests` | number, title, state, branches, head revision, linked task, synced at |
| `pull_request_syncs` | repository (host / owner / name) and when its open pull requests were last read; `pull_requests` rows of a repository are replaced with every sync (a cache of GitHub, desktop-written) |
| `pr_revisions` | pull request, revision, diff blob, synced at |
| `feedback_threads` | pull request, conversation, mode (question, fix), base / head revision, file, side, line range, excerpt, target profile, state |

### Access

| Table | Purpose |
| --- | --- |
| `approvals` | tool version, environment, run, step, action, revision, decision, decided by, expires at |
| `audit_events` | actor, capability, scope, run / step, decision, outcome — never secret values |

### Blobs

| Table | Purpose |
| --- | --- |
| `blobs` | sha256, size, stored size, media type, created at |
| `revision_diffs` | base revision, head revision, diff blob, read at — the diff between two commits read with git, e.g. a run's changes up to a checkpoint (a cache, desktop-written) |

Files are written to a temporary name and renamed after the hash is verified; the row is
inserted after the file exists, by the transaction that refers to the blob, which begins only once the
file is written. Blobs are never deleted (full retention).

## Write ownership

SQLite serializes writes between the two processes; ownership keeps meaning unambiguous.

- **Desktop** writes user-authored data: configuration, connections, secret metadata, tasks, run
  merges (`run_merges`, a person's integration of a finished run), environment additions,
  dependencies, run groups, user messages, feedback threads, approval decisions, settings.
- **Runner** writes execution data: runs, attempts, steps, plans, checkpoints, events,
  evidence, artifacts, usage, agent messages, message delivery state, audit events.
- Neither process edits the other's rows. A user request that changes execution (pause, cancel,
  continue) is an IPC command; the runner applies it and records the event. An answer to an agent's
  question with suggested answers is one too (`AnswerQuestion`): the runner records it, and for a
  question that ended its step it also writes the person's message that carries the answer to the agent.

## Concurrency

One project database has many connections at once: each run's task and each of its agents' workers in the
runner, the app's reads and writes (a connection per operation), the Orchestrator chat, the MCP servers, the
base-move follower, the plan-limit reads and the notifier. SQLite lets one of them write at a time. Every
connection is opened by `orchester_store`'s `migrate` (there is no other `Connection::open` in production
code), so these rules hold for all of them:

- **WAL, commits without a flush.** The databases are in WAL mode with `synchronous = NORMAL`: a commit
  appends to the log and lets go of the write lock without waiting for the disk; a checkpoint flushes. A
  crash of Orchester loses nothing committed; a power cut can lose the last commits, never consistency. With
  `FULL`, each commit flushed the log while holding the write lock, and on Windows a flush waits for what the
  whole drive still has to write, the agents' builds included: in ORC-74's stress test (eight runs, readers,
  a build writing beside them, on the drive of the real project) write transactions held the lock 39–61 % of
  the time over 6–8 s, the longest 24–135 ms, and a writer waited up to 1.1 s; on 2026-10-21 six or seven
  runs working for hours pushed that past the five seconds a writer then waited, and their writes failed with
  `SQLITE_BUSY` ("database is locked"), interrupting them. With `NORMAL` the same test holds the lock 7 % of
  the time, the longest 14 ms, and no writer waits more than 33 ms.
- **Write transactions are `IMMEDIATE` and short.** Every connection that may write (all but those opened
  only to read another project's database) begins its transactions with
  `BEGIN IMMEDIATE`: a transaction takes the write lock before it reads, so it waits for the lock at its start
  and never fails later on a read it upgrades (`SQLITE_BUSY_SNAPSHOT`, which SQLite does not wait for). A
  transaction holds only SQL: no git, file, process or network work runs while it is open (a blob's file is
  written before its transaction begins). A read outside a transaction ends with its statement, so no idle
  connection keeps a read open that holds checkpoints back.
- **Waiting for a lock.** Each connection's busy handler (`busy::wait_for_lock`) waits for a lock with short,
  growing pauses (1 ms up to 50 ms) for up to a minute in total before the statement fails with `SQLITE_BUSY`.
  This is the retry of every write, the app's (answers, merges, settings) as much as the runner's: nothing has
  run in the transaction yet when it waits. The run's log (events with step results and checkpoints,
  transcript, invocations, step outputs) also runs its whole transaction again when a busy error reaches it
  after it began, with doubling pauses, within the same minute (`busy::retry`).
- **Past the minute.** A write that still finds the database locked gives up: the run-log writes with
  `StoreError::Unavailable` ("the database stayed locked for 60.0 s, so the write gave up: database is locked
  (SQLITE_BUSY)"), the others with the SQLite error, whose message names its extended code. The runner
  interrupts the run with `runner_failed` and that reason in its activity, as for any other storage failure
  (a damaged database, a full disk, a schema it does not know), which are never retried; the app shows that
  the database is locked.
- **What the log tells.** Every write transaction's hold of the write lock is logged at debug level under the
  target `orchester_store::lock` with the transaction's first write (`INSERT agent_events`) and how long it
  waited; a hold of a second or more, a wait that passes a second (then every ten seconds), a wait given up,
  a retried transaction and a deferred transaction that wrote are warnings, with the waiting statement and
  its thread, in the log of the process (runner or app) that met them.

## State machines

- **Task:** draft → queued → running → awaiting plan approval ⇄ running → ready for review →
  merged / done; any active state → paused, cancelled, failed. Blocked is derived from
  dependencies, not stored. Only draft → queued is stored on the task: `tasks.status`
  (`orchester_core::task::TaskStatus`) says whether a person submitted it, and stays `queued` once
  its run started or ended (a queued task that never started can go back to `draft`). Where the task
  stands after that is never stored: it is derived from its latest run and that run's merge
  (`orchester_core::run::TaskPhase::of`), and the task page, the task list's run labels, the Needs
  you tab, the Queued count and the MCP `phase` read it there. The workflow status the task list's
  tabs and the overview count (Backlog … Done, `task_organization`) is stored separately: a person
  sets it, and run events (start → In progress, ready for review → In review) and a merge (Done) set
  it too. No screen reads `tasks.status` for where a task stands; the MCP tools name it
  `submission`.
- **Run:** pending → running ⇄ paused → succeeded / failed / cancelled; running → interrupted
  (session ended) → running on continue; running → waiting for a person (retries exhausted, or a
  step broke again) → running on a person's retry or return, failed on their stop, or cancelled. Interrupted keeps its last checkpoint. A pause is
  requested first and recorded as paused once the active agent invocation ended and its work was
  committed. A run that reaches its token budget or time limit is paused as stopped at limit
  after its agent was ended and its work committed; raising the limit continues it. A succeeded
  run is reopened by a follow-up (Ask agent) → running → succeeded again, also when the follow-up
  fails. A failed run can continue through a recovery attempt (Send & continue); a cancelled run remains final.
  Either task can instead start its next run with Run again
  (`orchester_core::run::run_again`): from the first step, in a new worktree on a new branch
  (`…-run-<n>`) from the checkout's `HEAD`, with the latest published version of the task's pipeline
  and the task's limits, and the project's next run number. It is refused while another run of the
  task has not ended, while the task's pipeline has no published version and while today's budget
  is used; a repeated request with the same key finds the run the first one started.
- **Step execution:** pending → running → waiting (approval, plan, message) → passed / failed /
  skipped.
- **Message:** draft (desktop only) → queued → delivered → acknowledged; failed keeps the draft.
- **Feedback thread:** draft → queued → working → answered / ready to verify → resolved;
  resolved can be reopened.
- **Approval:** requested → allowed / denied → expired.

Transitions live in `orchester-core` as typed functions; stores persist the result and the event.

## Open questions

1. IPC transport details, framing and versioning between desktop and runner.
2. Agent state inside checkpoints: what each provider can actually resume from.
3. Keychain naming is decided (`secrets` above); a moved project folder keeps its values. *Open:* what a project
   shows when its database is used on a computer whose keychain does not hold the values.
