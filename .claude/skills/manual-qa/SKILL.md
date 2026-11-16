---
name: manual-qa
description: How an agent runs manual QA of the Orchester desktop app as a person would — in a QA copy it started, through the window only (lurq window tools), making data through the app's own forms or by asking the Orchestrator chat, never through the app's domain MCP tools; local projects on scratch repositories and team projects against a team server run in Docker from the latest Docker Hub image. Use this skill whenever the person asks to "QA", "test it live", "lead a module to the end", "check it in the app", or to try a flow end to end in solo and team projects. Not design verification (that is `design-verification`), not automated tests.
---

# Manual QA

Manual QA proves a flow works the way a **person** meets it: the same windows, the same forms, the same chat, the
same agents. Its value is in the path, not only in the end state. A step done through a shortcut the person does not
have proves nothing about that step, and hides the bugs on the way to it.

## The rules

1. **Only the window.** Act through the app's window tools: `lurq_read_tree`, `lurq_screenshot`, `lurq_interact`
   (`click`, `double_click`, `type`, `key`, `paste`, `wheel`, `scroll_to`, `drag`), `lurq_set_value`. Reach screens
   through the sidebar, tabs and *Jump to*, as a person does.
2. **No domain tools that change anything.** Never call the app's MCP tools that make or change data or act on work:
   `add_project`, `create_module`, `update_module`, `create_task`, `update_task`, `run_task`, `merge_run`,
   `deliver_module`, `start_acceptance`, `create_pipeline`, and their like. Make data through the app's forms, or ask
   the **Orchestrator chat** for it in a conversation, as the person would (that is QA of the chat too).
3. **Reading is fine.** Read-only tools (`list_*`, `get_*`, `get_standing`, `get_watches`) and a read-only open of the
   project's `.orchester/orchester.db` may check results and feed a monitor. They never replace a step.
4. **A QA copy, not the installed app.** Run QA in a QA copy you started (`tools/qa-app.ps1`, or the `qa_app` example
   built from the branch under test, copied into `.tmp/qa/<task>/bin/`). In a QA copy the MCP client counts as the
   person for the whole window (`docs/stable-build.md`, "QA copies count the MCP client as the person"), so
   person-only steps (*Lead to the end*, *Authorize*, *Take over*, Settings) are yours to press. The installed app
   (`E:\Orchester`) refuses them to automation on purpose: never work around that refusal, never touch its database
   or the person's real projects. If the person asks for a check in the installed app, its person-only steps are
   theirs: ask, naming exactly where to press.
5. **Never global input, never the whole screen.** A native dialog of the QA copy (the folder picker) is driven only
   as a window of that process, found by its pid with UI Automation: set its field with `WM_SETTEXT` and press its
   button with `BM_CLICK`. Nothing else on the desktop is touched.
6. **Scratch data only.** Local projects are scratch repositories under `.tmp/qa-*/`; team projects live on a team
   server you started for the check (below). Whatever QA makes stays out of the person's projects and servers.
7. **Real agents, chosen in the window.** Use the CLI the person names (Codex, Claude) and choose it where the person
   does: the composer's *Runs on*, the pipeline's step profiles. Small tasks with clear criteria keep a run short.

## Gotchas of the window tools

- `lurq_set_value` sets a text field but opens no slash menu: type a `/command` with `lurq_interact` `type`, then pick
  it from the menu.
- `lurq_interact` `press` holds the mouse button down (release it with `release`); a keyboard key is `key`, e.g.
  `{"action": "key", "key": "Escape"}`.
- Elements are found by their `#id` in `lurq_read_tree`; refs change with every frame, so read the tree right before
  each press. Long lists are virtualized: scroll before you conclude something is missing.
- A press lands on coordinates: an element under the fold (a long form's last switch, its *Publish*) is not pressed
  until `lurq_interact` `scroll_to` with its ref brought it into the window. An open select's options have no ids:
  read their boxes in the tree and click an option by `x` and `y`.
- A refusal answered `person_only` means you are not in a QA copy (or the place keeps its own refusal): stop and say so.
- A QA copy keeps its sign-ins in memory: after a restart it is signed out of the team server. Its entry in
  Settings › Orchester Cloud then offers *Join again* › *With a code*: compare the check code, then join with a
  fresh account code.

## A local project

1. Make a scratch repository: `git init -b main` under `.tmp/qa-<topic>/<name>`, a small program with tests, one
   commit.
2. Build the QA copy from the branch under test (`cargo build -p orchester-app --example qa_app` and
   `-p orchester-runner --bin orchester-runner`), copy both into `.tmp/qa/<task>/bin/` (the app as
   `orchester-app-qa.exe`), and start it with `ORCHESTER_APP_MCP_START_WITHOUT_FOCUS=1`,
   `ORCHESTER_APP_MCP_WINDOW_CONTROL=on`, `ORCHESTER_APP_MCP_DISCOVERY=.tmp/qa/<task>/mcp` and `--signed-out`. Keep
   its pid; stop it by that pid (`CloseMainWindow`) when done. `tools/qa-app.ps1` does the same with a seeded project.
3. Add the project: *Choose folder…*, then the folder dialog as in rule 5. The dialog may hand over a subfolder it
   had selected: the app refuses it and names the repository's folder; choose again.
4. Set up through the window or the chat: pipelines, agents, the module and its tasks. Asking the Orchestrator chat
   ("make a module X on its own branch, with these three tasks …, don't start anything") is the usual way, and its
   answer is itself under test.
5. Run the flow as the person would, e.g. `/watch-module` → *Lead to the end* → the module → Send.

## A team project

Run a team server of your own from the latest images on Docker Hub, never the person's server:

1. Start Docker Desktop if its engine does not answer (`docker info`).
2. In `.tmp/team-server/`, copy `deploy/server/compose.private.yaml` from the platform repository
   (`H:\projects\orchester-cloud`; `deploy/server/README.md` there explains it), `docker pull
   ivanzaida/orchester-server:latest` and `ivanzaida/orchester-server-admin:latest`, and write `.env`:
   `ORCHESTER_VERSION=latest`, a random `ORCHESTER_ADMIN_SECRET` (32 bytes, base64), `ORCHESTER_ADMIN_CLOUD_URL=`
   (empty: no cloud), `ORCHESTER_SERVER_REPORTS=off`, `ORCHESTER_SERVER_TLS_NAMES=localhost,127.0.0.1`.
3. `docker compose -p orchester-qa -f compose.private.yaml up -d` (check that port 8740 is free first). The server's
   log (`docker compose -p orchester-qa -f compose.private.yaml logs server`) prints the certificate's check code and
   the setup code, which makes the first admin.
4. What the server's operator does in its admin is not the app under test and is done over the server's API: the
   first admin (`POST /api/v1/auth/code` with the setup code and `kind: "admin_web"`), and a project
   (`POST /api/v1/projects` as that admin). A code entered in an app needs an identity the server can check: start
   the QA copy `--signed-in` (its stand-in cloud signs with the test root of `orchester-api`'s `cloud_keys`) and pin
   that root on the server (`orchester-server cloud pin <root public key>`). Then, in the QA copy, join with an
   account code (`orchester-server account-code <username>`) through Settings › Orchester Cloud › *Connect a server*,
   comparing the check code with the server's log; add the scratch repository as a project and link it in Project
   settings › Sharing; make the team's profile and pipeline in the Agents and Pipelines hubs, and the module and its
   tasks through the chat.
5. Run the same flow as on the local project. Where the team path differs by design, the row in
   `docs/design/local-and-team-parity.md` says so; anything else that differs is a finding.
6. When done: `docker compose -p orchester-qa -f compose.private.yaml down -v`.

## Watching a long flow

Agents take minutes. Do not sleep in the foreground: start a background script that polls with read-only tools (or
the project database opened read-only) and prints each change of the tasks, runs and module, and exits on the end
state, on something waiting for the person, or on a time limit; then wait for it. Read the conversation's entries at
the end: every turn, what started it, what it did and why.

## The report

Tell the person, in their language:

- what was run and how (which copy and build, which projects, which agents), and whether the flow reached its end;
- what the app did at each step, with time and tokens when they matter;
- **findings**, numbered and ranked, each with what happened, where, and what was expected; a finding in the
  path counts even when the end state is right;
- what was not checked and why (a state you could not reach, a step only the person can do);
- what is left behind (scratch repositories, branches, worktrees, the QA copy, the Docker stack) and what you stopped.

Do not fix findings in the middle of QA unless the person asks; finish the flow first, so one report holds them all.
