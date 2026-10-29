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

## Open questions

1. ~~What existing profiles migrate to.~~ Decided in ORC-343: every existing profile is Manual (profile schema 5; the
   mode is left out of the stored form while Manual, so older versions read as Manual and are not rewritten), its
   commands kept as they were: *Allowed commands* keeps its list as Manual's, *Ask before running commands* keeps
   asking (no list), *Any command in the worktree* stays as it was, Manual too, until the person switches it to
   Autopilot.
2. Whether a run switched to Manual for itself may go back to Autopilot while its profile and project allow it (not
   drawn; the chip just reads *Manual · this run*).
3. Whether *Approved by autopilot* rows group per step (*… and 3 more*) or always list each request.
4. The exact Claude Code and Codex options that turn the CLI's own prompt off while still handing requests outside
   the worktree to Orchester, and which CLI versions can't (07d's *Not enforced* row is illustrative).
5. Whether the old *Approved for every run* rules apply in Autopilot to requests that still ask (outside the
   worktree), or only in Manual.
