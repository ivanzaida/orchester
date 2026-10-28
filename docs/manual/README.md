# Orchester manual

Orchester is a desktop app (Windows and macOS) for orchestrating coding agents per project. You describe work as
**tasks**; each task runs through a **pipeline** of steps carried out by **agents** (Claude Code or Codex) in their own
git worktrees; Orchester tracks every run, asks you only what its rules leave to you (approvals, questions, plans,
decisions), and delivers the result into your branch by a merge, the merge queue, a pull request or a report.
Everything — tasks, runs, pipelines, agent profiles, instructions, skills, tools, secrets and connections — belongs to
one project and lives in that repository's `.orchester/` folder.

Everything you can do in the window is also available to automation through Orchester's **MCP server**, with the same
rules as the screens and person-only switches for anything that widens what agents may do.

This manual describes the current app (`crates/desktop-app`, binary `orchester`, version 0.3.x).

## Contents

| Document | What it covers |
|---|---|
| [concepts.md](concepts.md) | Projects, tasks, runs, pipelines, agents, connections, accounts, limits, approvals, merging, memory, cloud — every term defined |
| [user-guide.md](user-guide.md) | The window, every screen, and the workflows from adding a project to merging a run |
| [settings.md](settings.md) | Every app and project setting with its default |
| [install-and-platforms.md](install-and-platforms.md) | Installing, data locations, Windows/macOS differences, known gaps, what is not built yet |
| [orchestrator-chat.md](orchestrator-chat.md) | The project's Orchestrator chat: what it may do directly, what it proposes, its switches and its 83 tools |
| [mcp-server.md](mcp-server.md) | Orchester's MCP server: connecting, tokens, scopes, permission gates, errors, the window tools (`app_state`, `navigate`, `lurq_*`) |
| [mcp-tools-setup.md](mcp-tools-setup.md) | MCP reference — project setup tools (project, connections, accounts, MCP servers, profiles, pipelines, instructions, skills, tools, secrets, merging) |
| [mcp-tools-work.md](mcp-tools-work.md) | MCP reference — work tools (tasks, modules, runs, queue, approvals, questions, plans, messages, pull requests, usage, delivery, reports) |
| [agent-run-tools.md](agent-run-tools.md) | The tools agents get inside runs (`read_file`, `edit_file`, `run_check`, `start_qa_app`, `save_memory`, …) |

## Quick start

1. Install Orchester ([install-and-platforms.md](install-and-platforms.md)), plus git and an agent CLI (Claude Code or
   Codex) signed in.
2. Start the app and **sign in** to Orchester Cloud.
3. **Add project**: choose the top folder of a git repository.
4. **Project settings › Connections › Add provider**: a Claude Code or Codex CLI connection.
5. **Agents › New agent**: create and publish profiles (e.g. Explorer, Planner, Builder, Reviewer).
6. **Pipelines › New pipeline › Starter · Feature delivery**: assign the profiles and publish it.
7. **Tasks › New task**: a title, the outcome, acceptance criteria and the *Pipeline* chip → **Create & run**.
8. Follow it in the task's **Conversation**; answer what waits for you (the **N need you** pill); merge from the
   **Summary** when it is ready.

To let your own agent drive Orchester: Settings › Automation › *Connect an agent*, then see
[mcp-server.md](mcp-server.md).

## MCP tool index

All tools of the app's MCP server. RO = read-only. Gates: **AP** = needs *Let automation change agent permissions* for
widening calls; **AA** = needs *Let automation answer approvals*; **WC** = needs window control.

### Window and app

| Tool | | Purpose |
|---|---|---|
| [`app_state`](mcp-server.md#app_state--read-only-always-listed-no-arguments) | RO | Where the window is: route, tabs, projects, runner, theme, window control, size |
| [`navigate`](mcp-server.md#navigate--needs-window-control) | WC | Take the active tab to a route |
| [`set_theme`](mcp-server.md#set_theme--needs-window-control) | WC | Dark, light or system |
| [`hold_reads`](mcp-server.md#hold_reads--debug-builds-only-needs-window-control) | WC, debug | Hold project reads to capture loading states |
| [`sign_in_state`](mcp-server.md#sign_in_state--read-only-always-answers) | RO | Whether Orchester is signed in to Orchester Cloud |
| [`lurq_screenshot`, `lurq_read_tree`, `lurq_inspect`, `lurq_find`, `lurq_find_by_id`, `lurq_find_by_class`, `lurq_windows`, `lurq_menu`, `lurq_wait`, `lurq_logs`, `lurq_profile_start`, `lurq_profile_read`, `lurq_profile_end`](mcp-server.md#looking-tools-always-available) | RO | Look at the window |
| [`lurq_act`, `lurq_set_value`, `lurq_interact`, `lurq_resize`](mcp-server.md#input-tools-need-window-control) | WC | Drive the window |

### Project setup ([mcp-tools-setup.md](mcp-tools-setup.md))

| Tool | | Purpose |
|---|---|---|
| `add_project` | | Add a project in a git repository |
| `get_project` | RO | A project's setup |
| `set_project_environment` | AP | Environment variables for agents and command tools |
| `set_repository_agent_files` | AP to turn on | Whether agents load `AGENTS.md`/`CLAUDE.md` |
| `list_connections` | RO | Agent connections, GitHub, MCP servers |
| `add_cli_connection` | AP for unusual programs / API providers | Add a Claude Code or Codex connection |
| `set_api_provider` | AP for new hosts/keys | Run a CLI connection on an API provider, or back on its sign-in |
| `check_connection` | | Check a connection again |
| `list_models` | RO | A connection's models and efforts |
| `list_accounts` | RO | CLI accounts and plan limits |
| `set_connection_account` | AP | The account a connection runs on |
| `add_mcp_server` | AP | Add an MCP server for agents |
| `update_mcp_server` | AP for widening | Change an MCP server |
| `remove_mcp_server` | | Remove an MCP server |
| `set_mcp_server_enabled` | AP to turn on | Turn an MCP server on or off |
| `list_profiles` / `get_profile` | RO | Agent profiles |
| `list_pipelines` | RO | Published pipelines |
| `save_profile_draft` | AP for widening | Save a profile draft |
| `publish_profile` | AP for broader/re-worded | Publish a profile |
| `get_pipeline` | RO | A pipeline with its steps and problems |
| `create_pipeline` | | New pipeline from the starter, empty or a copy |
| `save_pipeline_draft` | AP for weakening | Replace a pipeline's steps |
| `publish_pipeline` | AP unless no weaker | Publish a pipeline |
| `list_instructions` / `get_instruction` | RO | Instructions |
| `save_instruction_draft` | | Save an instruction draft |
| `publish_instruction` | AP | Publish an instruction |
| `list_skills` / `get_skill` | RO | Skills |
| `save_skill_draft` | | Save a skill draft |
| `publish_skill` | AP | Publish a skill |
| `list_tools` / `get_tool` | RO | Command tools |
| `save_tool_draft` | AP for new/widening | Save a command tool draft |
| `publish_tool` | AP for first/widening | Publish a command tool |
| `list_secrets` | RO | Secrets by name and use (never values) |
| `get_changes` | RO | What a run changed |
| `merge_run` | | Merge a run into its base branch |
| `complete_run` | AA with `anyway` | Complete a run that committed nothing |

### Work ([mcp-tools-work.md](mcp-tools-work.md))

| Tool | | Purpose |
|---|---|---|
| `list_projects` | RO | All projects |
| `set_code_index` | | Turn the code index on or off |
| `update_project_limits` | | Budgets, time limit, agent and QA slots |
| `get_auto_mode` | RO | Auto mode settings |
| `set_auto_mode` | AA | Change auto mode settings |
| `list_tasks` / `get_task` | RO | Tasks |
| `create_task` | | New draft task |
| `update_task` | AP to switch pipelines | Change a task, close or reopen it |
| `get_story_point_calibration` | RO | How story points tracked reality |
| `return_to_draft` | | Return a held queued task to Draft |
| `list_modules` / `get_module` | RO | Modules, burndown, forecast |
| `create_module` / `update_module` | | Manage modules |
| `run_task` | | Queue and start, or run again |
| `list_runs` / `get_run` | RO | Runs and their full state |
| `pause_run` / `resume_run` / `cancel_run` | | Control a run |
| `continue_run` | AA for retry/return/accept/resolve | Continue a stopped run |
| `set_run_auto_mode` | AA | One run's auto mode |
| `remove_worktree` | | Remove an ended run's worktree |
| `hold_queue` / `release_queue` | | Hold or release new starts |
| `list_approvals` | RO | Waiting tool requests and plans |
| `decide_approval` | AA | Approve once or deny |
| `list_questions` | RO | Waiting questions and message waits |
| `answer_question` | AA | Answer a question |
| `get_plan` | RO | A proposed plan |
| `decide_plan` | AA | Approve a plan or request changes |
| `send_message` | | Message a run's agents |
| `get_conversation` | RO | A task's conversation |
| `duplicate_pipeline` | AP unless identical | Copy and publish a pipeline |
| `list_pull_requests` / `get_pull_request` | RO | GitHub pull requests |
| `get_usage` | RO | Tokens and agent time |
| `get_delivery` | RO | Throughput, cycle/lead time, flow, forecasts |
| `get_daily_report` | RO | A day's report |
