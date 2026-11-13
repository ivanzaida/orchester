# OA-11 · The chat does not depend on *Let automation answer approvals*

- **Status:** todo
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
- The *Also needed* row goes for what the switch now covers.

## Done when

- Workspace tests for each tool: the chat driving with the setting off is carried out; the chat not driving
  proposes as today; another MCP client with the setting off is still refused (`--test orchestrator_tools consent`).
- `docs/design/orchestrator-toolset.md` (decisions 5 and 14), `docs/manual/orchestrator-chat.md` and
  `docs/manual/settings.md` say the setting is for outside clients. If the settings tab's *Also needed* row changes,
  that is a visible change: its frame and an independent design verification.
- `tools/check.ps1` passes.
