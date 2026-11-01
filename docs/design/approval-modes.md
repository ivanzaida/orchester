# Approval modes: Autopilot or Manual per agent profile

Designed for ORC-342 (2026-10-29) from the person's decision of the same day; the implementation is ORC-341. The
design source is `crates/desktop-app/design/orchester.pen`, read and changed only through the pencil MCP server. Every name, path, command and time in the frames is illustrative
(Kontur, Builder, Fixer, KON-12, dana), never shipped data. Exports at scale 1: `.tmp/design/ORC-342/<node id>.png`.

## Decisions

- **Each agent profile has an approval mode**, beside its other permissions (repository access, network, MCP
  servers):
  - **Autopilot** — every tool request is approved at once, without a card, except work outside the run's worktree,
    its Sources folders and the agent's own temp folders, pushes to a remote, and changes to refs other than the run's
    branch; those come to the person.
  - **Manual** — the profile's allowed commands (and what its file, network and MCP permissions allow) run at once;
    anything else goes first to the reviewer (the LLM of today's auto mode), which may approve something harmless;
    what it denies or is unsure about comes to the person.
- **Decisions always come to the person** in both modes: agents' questions, plans, a step that needs a choice or ends
  unverified, limits, merge conflicts. Autopilot never answers them. What answers one without a person today, and the
  setting that allows it (ORC-348; none of them reads auto mode, *approve everything in each run* or a run's
  *Approve everything in this run*, which answer tool requests only, `runner::execution::step::answer`):

  | Decision | Who answers it without a person | Setting that allows it |
  |---|---|---|
  | An agent's question | An MCP client: `answer_question`, the question card, the composer (window input) | Settings › Automation › *Let automation answer approvals* (`DecisionKind::Question`) |
  | An agent's question or message | The Orchestrator chat: `answer_question`, `send_message` (`answered_by_orchestrator`) | *Let the Orchestrator answer agents* **and** *Let automation answer approvals*; otherwise a proposal the person applies |
  | A plan | An MCP client: `decide_plan`, the plan card and bar, the composer's changes | *Let automation answer approvals* (`DecisionKind::Plan`); the Orchestrator chat never decides plans |
  | A step that used up its retries, broke again, waits unverified or unchanged | An MCP client: `continue_run`'s `allow_retry`, `return_to`, `accept`, the Summary's retry, return, stop and *Accept … as it is*, `complete_run` with `anyway` | *Let automation answer approvals* (`DecisionKind::Stuck`); the Orchestrator chat only proposes |
  | A finished run's work returned to a step | An MCP client: `continue_run`'s `return_to`, the Summary's *Return to … with note* | *Let automation answer approvals* (`DecisionKind::Stuck`) |
  | A limit (token budget, time limit, a step's budget) | An MCP client: `continue_run`'s `raise_limits`, `raise_step_budget`, the Summary's *Continue with …* and *Remove the limit* | *Let automation answer approvals* (`DecisionKind::Limit`, new in ORC-348: before, nothing gated them); the Orchestrator chat refuses them (`person_only`) |
  | The daily budget | An MCP client raising or removing it: `update_project_limits`'s `daily_budget`, Project settings › Limits' *Daily budget* (window input); then `continue_run`'s `resume` and `resume_run`, which raise nothing and go on only once the runner finds today's budget free again | *Let automation answer approvals* (`DecisionKind::Limit`, new in ORC-348: before, nothing gated it); lowering it takes no consent; the Orchestrator chat has no `update_project_limits` |
  | An account's plan limit | Waiting for the reset resumes by itself, as the person's default; continuing on another account by window input | *Let automation change agent permissions* |
  | Merge conflicts | The Deliver step's Integrate step sends them to its integrator agent (`merge_queue::integrate`); an MCP client: `continue_run`'s `resolve_conflicts`, the Summary's *Ask an agent to resolve* and *Resolve myself* | The pipeline's Deliver step (its integrator; none, and they go to the person); *Let automation answer approvals* (`DecisionKind::Stuck`) |
  | A merge waiting for approval | Only a person (`merge_queue::approve` refuses automation) | — |
  | Merging a finished run | The Orchestrator chat | *Let the Orchestrator merge runs*; on its own after a run's success also *merge finished runs* and *answer agents* |
  | Continuing an interrupted run | The Orchestrator chat | *Let the Orchestrator continue interrupted runs* |

  Not a decision, but still tied to auto mode: while *New runs use auto mode* is on, the Orchestrator chat's
  `run_task` starts a run the person asked for in the chat instead of proposing it (never in a turn nobody asked
  for). ORC-341 removes that setting; what replaces it there is open.
- **Project override.** Project settings › Runs › *Allow Autopilot for agents in this project*; off forces Manual for
  every agent. *New runs use auto mode* and *With auto mode on, approve everything in each run* go away; the reviewer
  setting stays (used by Manual) and so does *Approved for every run*.
- **Run.** A run can be switched to Manual for itself from its status line; never to Autopilot when its profile or
  project says Manual.
- **Widening.** Turning Autopilot on (profile or project) widens what agents do: it is asked in place at the field,
  like other widenings, and is person-only over MCP. Switching to Manual never asks.
- **Enforcement.** Orchester answers every tool request itself; the agent CLI's own permission prompt is turned off
  only for Autopilot. A CLI that can't hand its requests to Orchester never runs Autopilot: its runs use Manual and
  the profile says why.
- No modals: every setting here is a row on its page; the Autopilot widening is the existing in-place question.

## Enforcement

Built in ORC-347 (`orchester_core::permission::EffectiveApprovalMode`, `autopilot_support`; the runner's
`step::start`). Checked on this machine with Claude Code 2.1.301 and Codex 0.159.2 (2026-10-29), and against the
fake CLIs of the runner tests (`cargo test -p orchester-runner --features test-agent --test approval_mode_launch
approval_mode_launch`). Autopilot turns the CLI's own permission prompt off: its allow rules, its read-only command
classifier and its MCP rules decide nothing, and every tool request it makes reaches Orchester, which answers it. A
Manual step starts exactly as before. Bypass modes (`--dangerously-skip-permissions`, `--permission-mode
bypassPermissions`, Codex's `danger-full-access` or approval policy `never`) are never used: they would hand nothing
to Orchester.

| CLI | Autopilot options | Works from | Manual |
|---|---|---|---|
| Claude Code | `--mcp-config <relay config> --permission-prompt-tool mcp__Orchester__approve` and `--settings {"permissions":{"ask":["Bash"]}}`; no `--allowedTools "Bash(<prefix>:*)"` and no `mcp__<server>` rules; a writing step in `--permission-mode acceptEdits`, a read-only one in `default`. The whole-tool ask rule is checked before `acceptEdits` and the read-only classifier, so every command asks, and every call of the profile's MCP servers asks for want of a rule. Claude Code's own file tools stay on `--disallowedTools` (the agent uses Orchester's, which keep to the worktree); Orchester's own tools keep their `mcp__Orchester__<tool>` rules, as Orchester runs them itself | 2.1.89 (the oldest version `--permission-prompt-tool` and the whole-tool ask rule were checked with; `CLAUDE_CODE_AUTOPILOT`) | as before: allowed commands by rule, the rest through the prompt |
| Claude Code on DeepSeek (or another API provider) | the same: the prompt is the CLI's, whatever model API it talks to | as Claude Code | as before |
| Codex | the same `codex app-server` arguments; the thread's approval policy `{"granular":{"sandbox_approval":true,"rules":true,"mcp_elicitations":true,"request_permissions":false,"skill_approval":false}}` with `approvalsReviewer: "user"`, and no MCP tool exempt from approval. Every request Codex makes reaches Orchester: an escalation out of its sandbox, a command an execpolicy rule asks about, an MCP call that needs approval. Commands inside the sandbox make no request: the sandbox keeps their writes in the worktree and the writable folders | 0.159.0 (the oldest version App Server accepted the granular policy with the user as reviewer; `CODEX_AUTOPILOT`). Older versions lack it, so Orchester's App Server path does not support them for Manual either | as before: `rules` only with *Ask before running commands*, `mcp_elicitations` only with *Ask before MCP actions* |
| Codex on an API provider | the same | as Codex | as before |
| Native agents (Anthropic API, OpenAI API) | nothing to turn off: Orchester runs every tool call itself | every version | as before |

What the CLIs still do by themselves in Autopilot: Codex reads outside the worktree and, with network on, reaches the
network from inside its sandbox without a request; the user's own Codex execpolicy `allow` rules run a command outside
the sandbox unasked (as in Manual, `docs/interactive-tool-approvals.md`); Claude Code's web tools, offered only with
network on, ask per domain and are refused by the prompt as in Manual.

**Can't enforce.** A CLI older than the version above, or one no check has found the version of, never runs
Autopilot: the step runs as Manual (`EffectiveApprovalMode::Manual` with `ManualReason::CantEnforce`), its launch
recorded as Manual, and the runner logs why. The profile's Connection enforcement reads it from
`AgentConnection::autopilot_support()`: `AutopilotSupport::Enforced(<mechanism>)`, or
`AutopilotSupport::CantEnforce(CliTooOld { installed, required } | VersionUnknown { required })` for 07d's *Not
enforced · runs as Manual* row (the UI is a separate task).

Because any Autopilot step can fall back so (a later check can find an older CLI), an Autopilot profile's own
commands setting must be enforceable on its connection as in Manual: `AgentPermissions::enforced_on` refuses it
otherwise, so preflight, saving a profile and a run's permission change refuse what the fallback could not start. On
Claude Code this refuses *Any command in the worktree* in Autopilot too, as in Manual (it cannot confine commands).

## Answering

Built in ORC-346 (`orchester_core::run::{autopilot_step, after_review, RequestAnswer, CameToPerson}`, the runner's
`step::answer` and `step::approval::route`). A rule a person made in the run (Approve similar, Always allow) answers in
both modes, and so does a tool its MCP server lists to run without asking (`always allowed`, recorded as `profile`;
ORC-610): Autopilot's CLI hands even those calls to Orchester, which approves them without the reviewer. Then:

- **Autopilot** places the request against the run's roots (`autopilot_check`: a command's parts, the folder a call of
  Orchester's tools reads in, Orchester's other tools as inside; a call of another MCP server's tool is `Unknown`, as
  its arguments may name any path and its tool may act on a remote service): `Inside` is approved by the rule, without
  the reviewer and without a card; `Outside` and `MainCheckout` come to the person as `outside` (with the path), `Push`
  as `push`, `OtherRef` as `refs` (with the ref); `Unknown` asks the reviewer one question (`ROOTS_QUESTION`, with the
  roots and the run's branch): its approval answers, its ask brings it to the person as `reviewerUnsure` with the
  reviewer's reason, and a review without a verdict as `reviewerCouldNotRun`. The step's own limits hold in Autopilot
  too (`review::step_limit`, the profile's permissions kept beside the mode): a credential read, a write in a read-only
  step, the network or an install while it is off, and permissions not recorded come to the person as `notAllowed`
  without the reviewer, inside the roots or not, Codex escalations included; a part Orchester could not read in a step
  that only reads or whose network is off goes to the reviewer's question, which also asks whether it does what the
  step's facts forbid. The run's own *Stop* of
  auto-approval takes Autopilot back to Manual's rules, as the status bar counts it.
- **Manual** approves what the profile allows (a command whose every part the permissions run), then a project rule
  (*Approved for every run*), then the run's or the profile's
  auto-approval; the rest goes to the reviewer after its hard rules. A hard rule brings it to the person at once (`push`,
  `outside` for a write outside the folders, else `notAllowed`), as does a project without a reviewer (`notAllowed`);
  the reviewer's ask is `reviewerDenied`, a review without a verdict `reviewerCouldNotRun`.
- A review without a verdict (its CLI failed, it timed out, its answer could not be read) says the reviewer couldn't
  run, never that it was unsure (ORC-611): the card says why — for a CLI that failed, what Orchester knows of the
  reviewer's account (its plan limit reached, with when it resets; nobody signed in), else that the CLI failed — with
  *Reviewer settings*, which opens Project settings › Runs. There the Reviewer row says, in its hint's place, when the
  reviewer's account cannot run or its last review failed, and the Estimates row when the estimator's last request
  could not run (`run::review::ReviewerHealth`, read by `Workspace::reviewer_health`).
- The reviewer's verdict stays *approve* or *ask*: on Manual's question an ask reads as a denial, on Autopilot's as
  doubt.
- Each request records how it was answered (`autopilot`, `profile`, `reviewer` with its reason, `person`, which
  includes automation answering in the person's place and the person's own rules) and, while it waits for the person
  and after, why it came (`tool_approvals.answered_as`, `came_to_person`); `get_run`'s `requests` and `list_approvals`'
  `came_to_you` show them.
- A request that came to the person by Autopilot's rules is looked at again by Manual's rules once its run drops to
  Manual (the worker reads the mode at its regular beat while such a request waits): it is no longer waiting for the
  person while the reviewer judges it. A review whose run changed mode meanwhile is dropped, and the request is answered
  by the mode it is in now.

## Frames

| Node | Board | Frame | What it shows |
|---|---|---|---|
| `OXFBX` | Agents · redesign (`ZsU3S`) | Row 7 · Approval modes (ORC-342) | The row holding the profile frames and the notes |
| `J1ixkr` | Agents | 07 · Permissions · Approvals · Manual | Approvals first after Repository access: Autopilot / Manual radio rows with one line each; Manual selected with *Allowed commands* (chips, add field) right under it and *Ask before MCP actions that require approval*; the decisions note; *Commands and network* becomes *Network*; enforcement row *Approval mode · Manual* |
| `VOpn8` | Agents | 07b · Permissions · Approvals · Autopilot | Autopilot selected with *Still asks you* (outside the worktree, Sources and temp folders · pushes to a remote · refs other than the run's branch), the in-place widening *Allow Autopilot for Builder?* as the last row inside the Approvals card (`E5U3S`, as `SqDvZ` sits in `01c`), draft bar *Approvals · Manual → Autopilot*, enforcement row *Approval mode · Autopilot* |
| `Ejpm9` | Agents | 07c · … · Autopilot · light | 07b in the light theme |
| `J0MveH` | Agents | 07d · Approval mode · connection enforcement | The enforcement row per CLI and mode: Claude Code and Codex, Autopilot and Manual (Enforced), an older Codex that can't hand requests to Orchester (Not enforced · runs as Manual) |
| `NiVRV` | Agents | Notes · approval modes | Decisions, what changed, what must be built, open questions |
| `m0eUB` | Project settings · redesign (`XGtkg`), Row 1 `BKCQT` | 02c · Runs · Approvals · Autopilot allowed | *Auto mode* → *Approvals*: *Allow Autopilot for agents in this project* on, the reviewer (connection, model, effort), *Approved for every run*; Estimates names the reviewer's model |
| `tvmsJ` | Project settings, Row 1 | 02d · Runs · Approvals · Autopilot not allowed | The switch off: *every agent runs as Manual · Builder and Fixer are set to Autopilot* |
| `f1qdPc` | Project settings, Row 1 | 02e · Agent profile · Autopilot not allowed by the project | How a profile set to Autopilot reads then: *Not allowed in Kontur* badge, Manual *Used in Kontur*, *Open Project settings › Runs* |
| `H6kPd` | Task page · redesign (`C3wC8`), Row 3 `J57jh2` | 04 · Conversation · Autopilot run | Status line with the *Autopilot* mode chip and *Switch to Manual for this run*; *Approved by autopilot* rows; the card *Builder asks to edit a file outside the worktree* with the path and its reason; primary action *Approve* |
| `p6tXIO` | Task page, Row 3 | 04b · Conversation · Manual run | Chip *Manual · this run*; rows *Switched to Manual for this run*, *Allowed by the profile*, *Approved by the reviewer* (its reason); the card for a command not on the allowed list with the reviewer's reason |
| `vZcoN` | Task page, Row 3 | 04c · Approval rows and cards | The chips (Autopilot, Manual, Manual · this run, Manual · project, Manual · profile), the four row kinds, and the cards: outside the worktree, not on the allowed list (reviewer denied), push to a remote, another branch |

Reused: the window `qlXhJ` with its body overrides copied from `01c` (`WAWHr`), `02 · Runs` (`b7Ya1u`) and
`02 · Conversation` (`gFpQU`); the radio, sub-block, chip-pill, note and in-place question rows of `01c`; the decision
card of `CqVv3`; the *What changed* panel of `T7qrB`; `Tag chip` `FK4CS` for the mode chip; `Badge/Warning`,
`Badge/Neutral`, `Button/Primary`, `Button/Secondary`, `Toggle/Switch on/off`. No component, variable or existing frame
was changed; `02 · Runs` and `01c` stay as the earlier design, superseded for their auto-mode parts by the frames above.
Gaps and paddings in the new frames use the `$spacing_*` variables where a value matches one (the 28 px window padding
and the copied rows' 2/6/10/14 px values have no variable). The Claude Code version reads 2.1.173 everywhere.

## Copy

Proposed keys (agents: `agents.json`; project settings: `projectSettings.json`; task page: `task.json`):

| Key | English |
|---|---|
| `agents.permissions.approvals.title` | Approvals |
| `agents.permissions.approvals.autopilot` / `.hint` | Autopilot / Approves every tool request at once. Work outside the worktree, pushes and other branches still ask you. |
| `agents.permissions.approvals.manual` / `.hint` | Manual / Allowed commands run at once. Anything else goes to the reviewer first, then to you. |
| `agents.permissions.approvals.allowedHint` | These run without asking, as do the profile's file, network and MCP permissions. |
| `agents.permissions.approvals.stillAsks` / `.hint` | Still asks you / Autopilot never approves these; each waits in Needs you with its reason. |
| `agents.permissions.approvals.stillAsks.{outside,push,refs}` | Outside the worktree, Sources and temp folders · Pushes to a remote · Refs other than the run's branch |
| `agents.permissions.approvals.decisions` | In both modes decisions come to you: questions, plans, a step that needs a choice or ends unverified, limits and merge conflicts. |
| `agents.permissions.approvals.widen.title` / `.reason` | Allow Autopilot for {profile}? / Autopilot approves this agent's tool requests in the worktree without asking you. |
| `agents.permissions.approvals.notAllowed` / `.hint` | Not allowed in {project} / {project} doesn't allow Autopilot, so {profile}'s runs there use Manual. The choice stays for projects that allow it. |
| `agents.enforcement.approvalMode.{autopilot,manual}` | Orchester answers every request; {cli}'s own prompt is off · Orchester answers: allowed list, then the reviewer, then you |
| `agents.enforcement.approvalMode.cantEnforce` | Can't hand its requests to Orchester · runs as Manual until {cli} is updated |
| `projectSettings.runs.approvals.allowAutopilot` / `.on` / `.off` | Allow Autopilot for agents in this project / On · profiles set to Autopilot approve their own requests · only you can turn it on / Off · every agent runs as Manual · {profiles} are set to Autopilot |
| `projectSettings.runs.approvals.reviewer.hint` | For Manual requests off the allowed list |
| `task.run.mode.{autopilot,manual,manualRun,manualProject,manualProfile}` | Autopilot · Manual · Manual · this run · Manual · project · Manual · profile |
| `task.run.mode.switchToManual` | Switch to Manual for this run |
| `task.conversation.approval.{autopilot,profile,reviewer}` | Approved by autopilot · Allowed by the profile · Approved by the reviewer |
| `task.conversation.approval.switched` | Switched to Manual for this run |
| `task.request.reason.{outside,notAllowed,reviewerUnsure,reviewerDenied,push,refs}` | Came to you: the path is outside the worktree, its Sources and {agent}'s temp folders. Autopilot never approves these. · Not on {agent}'s allowed list. · The reviewer wasn't sure: "{reason}" · The reviewer denied it: "{reason}" · Pushes to a remote always come to you, in Autopilot too. · Only the run's branch {branch} is the run's own; other refs come to you. |
| `cards.approval.asks.{outside,notAllowed,push,refs}` (ORC-354) | {agent} asks to edit a file outside the worktree · {agent} asks to run a command not on its allowed list (the reviewer unsure or denying it too) · {agent} asks to push to a remote · {agent} asks to change another branch; for an MCP call `.outsideTool` / `.notAllowedTool`: … to reach outside the worktree · … to call a tool not on its allowed list |
| `cards.approval.asks.meta`, `cards.approval.moreAnswers`, `cards.approval.quiet.listedAt` (ORC-354) | {step} · {time} · More answers (the card's other answers, folded under *Approve* and *Deny*) · · on the allowed list · {time} |

## Open questions

1. ~~What existing profiles migrate to.~~ Decided in ORC-343: every existing profile is Manual (profile schema 5; the
   mode is left out of the stored form while Manual, so older versions read as Manual and are not rewritten), its
   commands kept as they were: *Allowed commands* keeps its list as Manual's, *Ask before running commands* keeps
   asking (no list), *Any command in the worktree* stays as it was, Manual too, until the person switches it to
   Autopilot.
2. Whether a run switched to Manual for itself may go back to Autopilot while its profile and project allow it (not
   drawn; the chip just reads *Manual · this run*).
3. ~~Whether *Approved by autopilot* rows group per step (*… and 3 more*) or always list each request.~~ Decided in
   ORC-354: Autopilot's rows that follow one another in a step share one row of the chat, the first three shown and
   the third saying *· 3 more*, which a press opens (`timeline::view::Row::Approvals`). A request's quiet row follows
   the tool group its call is in rather than splitting it. The app's words are `cards.approval.quiet.*` and
   `cards.approval.cameToYou.*` (the area files' rule), with *Approve* as `cards.approval.approve`; the seed's
   `cards-approval-modes` scenario shows every row kind and card reason.
4. ~~The exact Claude Code and Codex options that turn the CLI's own prompt off while still handing requests outside
   the worktree to Orchester, and which CLI versions can't.~~ Decided in ORC-347: see
   [Enforcement](#enforcement).
5. ~~Whether the old *Approved for every run* rules apply in Autopilot to requests that still ask (outside the
   worktree), or only in Manual.~~ Decided in ORC-346: only in Manual (see [Answering](#answering)).
