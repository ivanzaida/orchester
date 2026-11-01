# Settings consolidation: one place for every setting

Designed 2026-11-01 from the person's request ("some settings are doubled or split across project/app settings and
agent profiles") and their decisions of the same day. The design source is `crates/desktop-app/design/orchester.pen`,
board `Settings consolidation · redesign` (`zILeT`), read and changed only through the pencil MCP server. Every name,
path and value in the frames is illustrative (Kontur, dana-k, KON), never shipped data.

## Decisions

- **One place per setting.** Each setting is set in exactly one place. Anywhere else it is a read-only value with a
  link to where it is set, as a secret's page already shows the tools that use it.
- **Settings (the app) is the person and this computer:** General (appearance, the plan limits' warning threshold,
  privacy, updates, this computer), Notifications (with the Orchestrator's daily report), Accounts (agent CLI
  accounts and their plan limits), Automation (the MCP server, the window, *Let automation answer approvals* and
  *Let automation change agent permissions*), Browser, Orchester Cloud. Nothing about how a project's agents behave.
- **Project settings is what the project's agents get and may do**, in seven tabs:
  General · Runs · Orchestrator · Context · Connections · Environment · Sharing.
  - General (`S01` `WHjIP`): the project and its repository. *Repository agent files* moves to Context.
  - Runs (`S02` `Ekiul`): Approvals (Allow Autopilot, Reviewer, Approved for every run), Estimates, Limits. Limits
    says a task's run budget and time limit and a step's budget override these defaults. Environment moves out.
  - Runs with a decision model reviewing (`S02b` `X7Cz0W`): the reviewer row of `decision-reviewer.md` — connection,
    model and *Approves at 0.80 or above* in place of an effort, where requests go — and Estimates on a language model
    connection, as the decision reviewer writes no reasons.
  - Orchestrator (`S03` `X8mFTS`, new): *The Orchestrator may* — publish drafts, answer agents, merge runs, merge
    finished runs on its own, continue interrupted runs, write with git — **moved from Settings › Automation and made
    per project** (person's decision 2). Still the person's alone in the window. Existing app-wide values are copied
    into every project once; a new project starts with all off. Answering agents still also needs *Let automation
    answer approvals*, which stays per computer and shows here read-only with *Open*.
  - Context (`S04` `oTBHM`): everything agents get besides the worktree — Sources, Repository agent files, Code
    index, and **Memory** (saving of task and project records, records and tokens per invocation), **moved from the
    Memory page's Settings tab** (person's decision 3). The Memory page loses that tab and links to Context (`S15`
    `BJQn0`).
  - Connections (`S05` `yIMQy`): Agent providers, **Decision models** with *Add decision model* (the connection kind
    of `decision-reviewer.md`), GitHub and MCP servers.
  - Environment (`S06` `EnL2D`, was Secrets): Variables (from Runs › Environment) and Secrets in one tab; a decision model's API key is one of its secrets.
  - Sharing (`S07` `z4pzpm`): unchanged but for the tab row.
- **The agent profile chooses within the project's bounds:** role, connection and model, approval mode, commands,
  **writable folders (kept in the profile**, person's decision 1), network, browser sites, MCP servers. Under
  Connection enforcement it shows *Set by the project* read-only — Autopilot, Sources, Repository agent files, Code
  index, Memory, Environment, Limits, and the project's Reviewer — each row opening its tab (`S14` `o9GsNS`); that group takes over the
  enforcement column's Repository agent files row. The writable folders' hint says that reading needs nothing there,
  since every agent reads the project's Sources.
- **Settings, tab by tab:** General (`S08` `QsTjX`), Notifications (`S09` `aBn6v`), Accounts (`S10` `FajQF`), Browser
  (`S12` `WF4I9`) and Orchester Cloud (`S13` `vbkxY`) are unchanged but for the six-tab row, which the older boards
  drew without Browser.
- **Settings › Automation** (`S11` `V8aAj`) keeps the MCP server, the window and Approvals and permissions; in place of
  *The Orchestrator may* one row says each project sets its own and opens Project settings.

## The decision reviewer

`Decision reviewer · redesign` (`a6B6v`, `decision-reviewer.md`, proposed 2026-10-31) is part of this layout: its
reviewer row is Runs › Approvals' (`S02b`), its connection kind a group of Connections (`S05`), its key a secret in
Environment (`S06`), and the profile shows the reviewer read-only (`S14`). Its add form, connection page, approval cards
and the team's reviewer stay as that board draws them; their old tab row reads as this board's.

## Duplicates removed

- The app-wide Estimates value a project without its own falls back to: copied into such projects once, then dropped.
- The profile's pre-approval-modes *Approve automatically*.
- Usage › Plan limits' menu no longer changes the warning threshold in place; it opens Settings › General.

## Not in this design

The pages under Connections, Sharing, Secrets and Accounts (a connection's, a secret's, an account's); values a team sets for a shared project (as
`Shared configuration · redesign` draws them); the MCP tools' names and refusals, which follow the settings to their
new owners in the implementation.
