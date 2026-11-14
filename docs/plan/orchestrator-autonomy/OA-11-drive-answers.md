# OA-11 · The chat does not depend on *Let automation answer approvals*

- **Status:** done (2026-11-14); the settings tab no longer shows *Also needed*
- **Kind:** workspace
- **Depends on:** —
- **Branch:** —
- **Tokens:** —

## Goal

With *Let the Orchestrator drive* on, the chat answers what its agents ask without a second switch the person did not
know they needed.

## Today

- A tool request: the drive switch is the chat's consent since 2026-11-11 (`crates/workspace/src/app_mcp/tools/approvals.rs`).
- A question: `answer_question` sent by anyone but a person requires *Let automation answer approvals*
  (`DecisionKind::Question.require`, `app_mcp/tools/questions.rs`), and so does a `send_message` that answers or
  continues a run (`messages.rs`) and `decide_plan` (`plans.rs`). The chat is woken for the wait and is refused
  `automation_not_allowed`.
- The settings tab lists it under *Also needed*.

## Decided (2026-11-13)

The person: "remove the Orchestrator's dependence on the switch; that switch is only for outside MCP clients."

- While the chat drives, `answer_question` and a `send_message` that answers are its own, as a tool request's answer
  is: no second switch. Other MCP clients still need it.
- A plan: `decide_plan` is the chat's own while it drives, with its reason kept (OA-22). A message that would
  request a plan's changes stays refused (`plan_decision_person_only`): the plan is decided with `decide_plan`.
- Every other place a call of the chat requires that setting (`DecisionKind::…require`; today also raising the
  project's limits, `app_mcp/tools/limits.rs`) reads the drive switch for the chat instead. The task lists each place
  it found and what it does now.
- While the chat does not drive nothing changes: its changes are cards the person applies.
- **Standing policy too** (the person, 2026-11-13): choosing the reviewer (`set_reviewer`) and publishing a profile
  that approves by itself (`setup/widening.rs`) need only the drive switch for the chat.
- `complete_run` with `anyway` is the chat's own while it drives, as accepting a step is (`DecisionKind::Stuck`).
- **The daily budget is the exception**, taken by the session and to be confirmed by the person: in a turn Orchester
  started by itself the chat does not change the project's daily budget (`person_only`), because that budget is what
  bounds spending while nobody watches. In a turn a person's message started, it does, with no second switch.
- The settings tab's *Also needed · set for this computer* section (`computer.rs`) is now wrong: it says answering
  agents acts only while that setting is on. Removing it is a visible change and waits for the person's frame
  (`frames.md` G).

## Done when

- Workspace tests for each tool: the chat driving with the setting off is carried out; the chat not driving
  proposes as today; another MCP client with the setting off is still refused (`--test orchestrator_tools consent`).
- `docs/design/orchestrator-toolset.md` (decisions 5 and 14), `docs/manual/orchestrator-chat.md` and
  `docs/manual/settings.md` say the setting is for outside clients. If the settings tab's *Also needed* row changes,
  that is a visible change: its frame and an independent design verification.
- `tools/check.ps1` passes.

## Review (2026-11-13)

An independent reviewer read the change. `chat_drives` is set in one place only (`orchestrator/acts.rs`, after the
switch is read for that call), so no outside client, synthetic input or run agent reaches it; an outside client's
behaviour is unchanged. Taken from the review: the two standing-policy gates went back to the setting (above); the
tools' descriptions the chat reads now say the setting is not asked of it (`orchestrator/told.rs`); stale statements
in `docs/design/approval-modes.md`, `settings.md`, `orchester-workspace.md` and `docs/manual/settings.md` were
corrected; a test for `decide_plan`.

**The person's answers (2026-11-13):**

1. The drive switch also covers choosing the reviewer and publishing a profile that approves by itself: both gates
   read it now (`approval_settings.rs` `gate`, `setup/widening.rs`). *Let automation change agent permissions* still
   stands for what widens.
2. The daily budget rule stays: never changed in a turn Orchester started by itself, raised with no second switch
   when a person's message asks.

Tests run: `orchester-workspace --test orchestrator_tools orchestrator_tools`, `--test automation_consent consent`,
`--test decision_policy decision`; `orchester-core --test orchestrator_waits orchestrator`, `--test orchestrator_leads
leads`; `orchester-app --test mcp_domain` for `orchestrator_chat::`, `orchestrator_guard::`, `orchestrator_sent::`,
`approval_settings::`, `completion::`, `gates::`, `decisions_reach_person::`, `orchestrator_moves::`,
`orchestrator_report_turn::`, `orchestrator_continue::`, `orchestrator_approval`: 63 passed, and
`approval_settings::approval_mode_effective_switch_allows_autopilot_by_a_persons_press_alone` timed out twice in that
run of 64 and passed alone and with its module (load; this change does not touch what it tests). `tools/check.ps1`:
OK. Not run: `complete_run` with `anyway` by the driving chat (no test), a turn with a real model, macOS (no `cfg`
code).

## The settings tab (2026-11-14, group G)

Project settings › Orchestrator no longer has the section *Also needed · set for this computer* with *Let
automation answer approvals*: the chat does not depend on that setting. Its code and strings are gone
(`screens/project_settings/orchestrator/computer.rs`); the tab ends with *Leading to the end* (OA-10).
