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
  unverified, limits, merge conflicts. Autopilot never answers them.
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
4. ~~The exact Claude Code and Codex options that turn the CLI's own prompt off while still handing requests outside
   the worktree to Orchester, and which CLI versions can't.~~ Decided in ORC-347: see
   [Enforcement](#enforcement).
5. Whether the old *Approved for every run* rules apply in Autopilot to requests that still ask (outside the
   worktree), or only in Manual.
