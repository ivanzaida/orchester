# Settings consolidation: one place for every setting

Designed 2026-11-01 from the person's request ("some settings are doubled or split across project/app settings and
agent profiles") and their decisions of the same day. The design source is `crates/desktop-app/design/orchester.pen`,
board `Settings consolidation · redesign` (`r3SE5K`), read and changed only through the pencil MCP server. Every name,
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
  - General (`S01` `O5TkIl`): the project and its repository. *Repository agent files* moves to Context.
  - Runs (`S02` `d2dUrR`): Approvals (Allow Autopilot, Reviewer, Approved for every run), Estimates, Limits. Limits
    says a task's run budget and time limit and a step's budget override these defaults. Environment moves out.
  - Orchestrator (`S03` `B9RMAb`, new): the Orchestrator's switches — first six (publish drafts, answer agents, merge
    runs, merge finished runs on its own, continue interrupted runs, write with git), **moved from Settings ›
    Automation and made per project** (person's decision 2) — and since 2026-11-08 (ORC-909) **one**, *Let the
    Orchestrator drive*: on, the chat acts itself with the app's tools and no proposal cards; off, it proposes. Still
    the person's alone in the window. A project that had *Answer agents* on starts with it on; a new project starts
    off. Answering agents' questions and messages still also needs *Let automation answer approvals*, which stays per
    computer and shows here read-only with *Open*. (The board still draws the six rows until the design follows.)
  - Context (`S04` `zZ3QY`): everything agents get besides the worktree — Sources, Repository agent files, Code
    index, and **Memory** (saving of task and project records, records and tokens per invocation), **moved from the
    Memory page's Settings tab** (person's decision 3). The Memory page loses that tab and links to Context (`S15`
    `GgZNC`).
  - Connections (`S05` `gYOtR`): Agent providers, GitHub and MCP servers.
  - Environment (`S06` `IVQp7`, was Secrets): Variables (from Runs › Environment) and Secrets in one tab.
  - Sharing (`S07` `tsFHB`): unchanged but for the tab row.
- **The agent profile chooses within the project's bounds:** role, connection and model, approval mode, commands,
  **writable folders (kept in the profile**, person's decision 1), network, browser sites, MCP servers. Under
  Connection enforcement it shows *Set by the project* read-only — Autopilot, Sources, Repository agent files, Code
  index, Memory, Environment, Limits, and the project's Reviewer — each row opening its tab (`S14` `YLuIW`); that group takes over the
  enforcement column's Repository agent files row. The writable folders' hint says that reading needs nothing there,
  since every agent reads the project's Sources.
- **Settings, tab by tab:** General (`S08` `ei7iI`), Notifications (`S09` `GZXXS`), Accounts (`S10` `AHGWO`), Browser
  (`S12` `cgicZ`) and Orchester Cloud (`S13` `C5P3xN`) are unchanged but for the six-tab row, which the older boards
  drew without Browser.
- **Settings › Automation** (`S11` `grpLX`) keeps the MCP server, the window and Approvals and permissions. It had a
  group, *The Orchestrator*, with one row that said each project sets its own and opened Project settings; ORC-909
  removed it, since the Orchestrator has one switch, in Project settings.

## Duplicates removed

- The app-wide Estimates value a project without its own falls back to: copied into such projects once, then dropped.
- The profile's pre-approval-modes *Approve automatically*.
- Usage › Plan limits' menu no longer changes the warning threshold in place; it opens Settings › General.

## Not in this design

The pages under Connections, Sharing, Secrets and Accounts (a connection's, a secret's, an account's); values a team sets for a shared project (as
`Shared configuration · redesign` draws them); the MCP tools' names and refusals, which follow the settings to their
new owners in the implementation; the decision model reviewer, which is the Decision reviewer module's alone.
