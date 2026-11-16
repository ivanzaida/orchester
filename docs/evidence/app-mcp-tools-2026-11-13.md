# App MCP tools by module — 2026-11-13

Source revision: `2ab1600da304e86818a821d9340406e8885bc1a4`. Scope: registrations under [workspace app_mcp/tools](../../crates/workspace/src/app_mcp/tools/mod.rs), including [setup](../../crates/workspace/src/app_mcp/tools/setup/mod.rs); excludes desktop window/QA tools and the runner's agent file tools.

There are **100 tools: 42 marked read-only and 58 that change data**. “Read” follows the registration's `read_only` annotation; “Write” includes persisted checks, run control, filesystem cleanup and team-server actions, even when permission gates can refuse a call. The three retired tools change nothing and return errors.

Groups below name the handler module relative to `crates/workspace/src/app_mcp/tools/`, rather than the sometimes separate schema/registration module. Main registrations live in `mod.rs`; setup registrations are collected by `setup::listed`, including `connection_specs`, `mcp_server_specs` and `team_fill_ins::reviewer::SPEC`.

## `approval_settings`

| Tool | Effect | What it does |
| --- | --- | --- |
| `get_approvals` | Read | Reads project approval modes, reviewer configuration and automation allowances. |
| `set_allow_autopilot` | Write | Maps the legacy Autopilot switch to manual or autopilot approval mode. |
| `set_approve_mode` | Write | Changes the project's approval mode subject to person-only escalation rules. |
| `set_reviewer` | Write | Chooses or clears the reviewer consulted before the person. |
| `set_run_manual` | Write | Switches a run's approvals to Manual for that run. |
| `get_auto_mode` | Read | Returns the retired error and directs clients to get_approvals. |
| `set_auto_mode` | Read | Returns the retired error and directs clients to set_allow_autopilot and set_reviewer. |
| `set_run_auto_mode` | Read | Returns the retired error and directs clients to set_run_manual. |

## `approvals`

| Tool | Effect | What it does |
| --- | --- | --- |
| `list_approvals` | Read | Lists agent tool requests waiting for a decision. |
| `decide_approval` | Write | Approves or denies a waiting tool request, optionally recording a denial message. |

## `continue_run`

| Tool | Effect | What it does |
| --- | --- | --- |
| `continue_run` | Write | Continues stopped work using an offered limit, retry, return, acceptance or conflict-resolution choice. |

## `conversation`

| Tool | Effect | What it does |
| --- | --- | --- |
| `get_conversation` | Read | Reads paginated task conversation entries, plans, questions and run notices. |

## `daily_report`

| Tool | Effect | What it does |
| --- | --- | --- |
| `get_daily_report` | Read | Reads a saved daily report or assembles the current report from stored facts. |

## `delivery`

| Tool | Effect | What it does |
| --- | --- | --- |
| `get_delivery` | Read | Reads delivery throughput, cycle and lead times, flow and forecasts. |

## `limits`

| Tool | Effect | What it does |
| --- | --- | --- |
| `update_project_limits` | Write | Changes project run defaults, daily budget and concurrency limits. |

## `messages`

| Tool | Effect | What it does |
| --- | --- | --- |
| `send_message` | Write | Sends instructions to a task's agents, answering waits or continuing or restarting work as its state permits. |

## `modules`

| Tool | Effect | What it does |
| --- | --- | --- |
| `list_modules` | Read | Lists project modules, dates, base branches and progress. |
| `get_module` | Read | Reads a module's details, tasks and burndown. |
| `create_module` | Write | Creates an open module with optional dates, description and base branch. |
| `update_module` | Write | Updates a module's details, state or base branch. |

## `pipeline_copy`

| Tool | Effect | What it does |
| --- | --- | --- |
| `duplicate_pipeline` | Write | Copies a published pipeline into a newly published pipeline, optionally substituting agent profiles. |

## `plan_read`

| Tool | Effect | What it does |
| --- | --- | --- |
| `get_plan` | Read | Reads a proposed plan, its versions and its decision. |

## `plans`

| Tool | Effect | What it does |
| --- | --- | --- |
| `decide_plan` | Write | Approves a waiting plan or requests changes with a note. |

## `projects`

| Tool | Effect | What it does |
| --- | --- | --- |
| `list_projects` | Read | Lists workspace projects, folders, limits and app links. |
| `set_code_index` | Write | Turns the project's code index on or off for future agents. |
| `list_pipelines` | Read | Lists published pipelines and their steps in one or all projects. |

## `questions`

| Tool | Effect | What it does |
| --- | --- | --- |
| `list_questions` | Read | Lists waiting agent questions and steps awaiting a message. |
| `answer_question` | Write | Answers a waiting agent question with selected options or free text. |

## `queue_hold`

| Tool | Effect | What it does |
| --- | --- | --- |
| `hold_queue` | Write | Holds new starts for a project or a task's reviewed group. |
| `release_queue` | Write | Releases a project or reviewed group's hold on new starts. |

## `return_to_draft`

| Tool | Effect | What it does |
| --- | --- | --- |
| `return_to_draft` | Write | Returns an eligible queued task whose run never started to Draft and Backlog. |

## `run_list`

| Tool | Effect | What it does |
| --- | --- | --- |
| `list_runs` | Read | Lists recent runs for a task, project or all projects. |

## `run_task`

| Tool | Effect | What it does |
| --- | --- | --- |
| `run_task` | Write | Queues or starts a task, or starts another run after its previous run ended. |

## `runs`

| Tool | Effect | What it does |
| --- | --- | --- |
| `get_run` | Read | Reads a run's steps, state, waits, usage, delivery, merge attempts and CI checks. |
| `pause_run` | Write | Requests a run pause at its agents' next checkpoint. |
| `resume_run` | Write | Resumes a paused run or continues an interrupted local run. |
| `cancel_run` | Write | Cancels a run that has not ended. |

## `setup/accounts`

| Tool | Effect | What it does |
| --- | --- | --- |
| `list_accounts` | Read | Lists this computer's agent CLI accounts without credentials. |
| `set_connection_account` | Write | Chooses the account a CLI connection uses for future runs. |

## `setup/api_providers`

| Tool | Effect | What it does |
| --- | --- | --- |
| `set_api_provider` | Write | Configures a CLI connection's API provider or restores its own sign-in. |

## `setup/complete`

| Tool | Effect | What it does |
| --- | --- | --- |
| `complete_run` | Write | Records completion and marks the task Done for an eligible successful run with no commits. |

## `setup/connections`

| Tool | Effect | What it does |
| --- | --- | --- |
| `list_connections` | Read | Lists agent, decision and project MCP connections with their last known status. |
| `add_cli_connection` | Write | Adds a named Codex or Claude CLI connection. |
| `check_connection` | Write | Checks a connection and persists its status, version and models. |
| `list_models` | Read | Reads models and supported reasoning efforts from a connection's available model information. |

## `setup/decision_connections`

| Tool | Effect | What it does |
| --- | --- | --- |
| `save_decision_connection` | Write | Adds or edits a decision-model connection. |
| `check_decision_connection` | Write | Fetches a decision connection's models and records the check result. |
| `remove_decision_connection` | Write | Deletes a decision connection when no project reviewer uses it. |

## `setup/instructions`

| Tool | Effect | What it does |
| --- | --- | --- |
| `list_instructions` | Read | Lists project instructions, scope, targeting and publication state. |
| `get_instruction` | Read | Reads an instruction's draft or latest published text and targeting. |
| `save_instruction_draft` | Write | Creates an instruction as published v1 or updates an existing instruction's draft. |
| `publish_instruction` | Write | Publishes an instruction draft for future agent prompts. |

## `setup/mcp_servers`

| Tool | Effect | What it does |
| --- | --- | --- |
| `add_mcp_server` | Write | Adds a project MCP server using a program or remote address. |
| `update_mcp_server` | Write | Updates a project MCP server's fields or supported team-server configuration. |
| `remove_mcp_server` | Write | Removes a project MCP server when no profile uses it. |
| `set_mcp_server_enabled` | Write | Switches an MCP server on or off for future agents, with team enabling reserved for the person. |

## `setup/merge`

| Tool | Effect | What it does |
| --- | --- | --- |
| `get_changes` | Read | Reads checkpoint and uncheckpointed run changes against its starting base. |
| `merge_run` | Write | Merges an eligible local run or requests and carries on its team module's delivery. |

## `setup/pipeline_steps`

| Tool | Effect | What it does |
| --- | --- | --- |
| `save_pipeline` | Write | Publishes supplied pipeline steps as the next version. |

## `setup/pipelines`

| Tool | Effect | What it does |
| --- | --- | --- |
| `get_pipeline` | Read | Reads a pipeline's latest published definition and steps. |
| `create_pipeline` | Write | Creates and publishes a pipeline from a starter or a copy. |

## `setup/profile_input`

| Tool | Effect | What it does |
| --- | --- | --- |
| `save_profile_draft` | Write | Creates a profile as published v1 or updates an existing profile's draft. |

## `setup/profiles`

| Tool | Effect | What it does |
| --- | --- | --- |
| `list_profiles` | Read | Lists agent profiles, publication state and readiness on this computer. |
| `get_profile` | Read | Reads a profile's draft or latest version, permissions and configuration. |
| `publish_profile` | Write | Publishes an existing profile draft as its next version. |

## `setup/project`

| Tool | Effect | What it does |
| --- | --- | --- |
| `add_project` | Write | Registers a Git repository as a workspace project. |
| `get_project` | Read | Reads project setup, limits, environment and automation settings. |
| `set_project_environment` | Write | Replaces environment variables added to the project's agents and command tools. |
| `set_repository_agent_files` | Write | Changes whether future agents load repository agent instruction files. |

## `setup/project_tools`

| Tool | Effect | What it does |
| --- | --- | --- |
| `list_tools` | Read | Lists pipeline tools and whether steps can run them. |
| `get_tool` | Read | Reads a command tool's draft or latest definition, users and revision. |
| `publish_tool` | Write | Publishes a command tool draft for pipeline steps. |

## `setup/secrets`

| Tool | Effect | What it does |
| --- | --- | --- |
| `list_secrets` | Read | Lists secret names, metadata and uses without reading or returning values. |

## `setup/service_input`

| Tool | Effect | What it does |
| --- | --- | --- |
| `save_service_draft` | Write | Creates or replaces a service draft. |

## `setup/services`

| Tool | Effect | What it does |
| --- | --- | --- |
| `list_services` | Read | Lists run services, programs, dependencies and publication state. |
| `get_service` | Read | Reads a service's draft or latest definition and revision. |
| `publish_service` | Write | Publishes a service draft after dependency and permission checks. |

## `setup/skill_input`

| Tool | Effect | What it does |
| --- | --- | --- |
| `save_skill_draft` | Write | Creates a skill as published v1 or updates an existing skill's draft and reference files. |
| `publish_skill` | Write | Publishes a skill draft for future runs. |

## `setup/skills`

| Tool | Effect | What it does |
| --- | --- | --- |
| `list_skills` | Read | Lists skills, scope, profile uses and publication state. |
| `get_skill` | Read | Reads a skill's body, reference-file metadata, versions and uses. |

## `setup/stacks`

| Tool | Effect | What it does |
| --- | --- | --- |
| `list_stacks` | Read | Lists named service sets and their publication state. |
| `get_stack` | Read | Reads a stack's draft or latest services and revision. |
| `save_stack_draft` | Write | Creates or replaces a stack draft's service list. |
| `publish_stack` | Write | Publishes a stack draft after checking its services and dependencies. |

## `setup/storage`

| Tool | Effect | What it does |
| --- | --- | --- |
| `get_storage` | Read | Reads disk-guard thresholds, available space and cleanable run folders. |
| `set_keep_free` | Write | Changes the project's minimum free disk space threshold. |

## `setup/team_fill_ins`

| Tool | Effect | What it does |
| --- | --- | --- |
| `pick_profile_connection` | Write | Saves this computer's connection and optional account choice for a team profile. |

## `setup/team_fill_ins/reviewer`

| Tool | Effect | What it does |
| --- | --- | --- |
| `pick_reviewer_connection` | Write | Saves this computer's compatible connection choice for a team reviewer. |

## `setup/team_mcp_list`

| Tool | Effect | What it does |
| --- | --- | --- |
| `list_team_mcp_servers` | Read | Lists team MCP servers and their readiness or missing configuration on this computer. |

## `setup/team_pipeline`

| Tool | Effect | What it does |
| --- | --- | --- |
| `share_pipeline` | Write | Makes an Admin's local pipeline a new team pipeline. |

## `setup/tool_input`

| Tool | Effect | What it does |
| --- | --- | --- |
| `save_tool_draft` | Write | Creates a command tool as published v1 or updates an existing tool's draft. |

## `task_write`

| Tool | Effect | What it does |
| --- | --- | --- |
| `create_task` | Write | Creates a draft task with its pipeline, limits and organization. |
| `update_task` | Write | Updates task details, organization or workflow status. |

## `tasks`

| Tool | Effect | What it does |
| --- | --- | --- |
| `list_tasks` | Read | Lists tasks and their workflow, organization and run status in one or all projects. |
| `get_task` | Read | Reads a task's brief, criteria, settings, runs and latest successful result. |
| `get_story_point_calibration` | Read | Reads merged-task story points against actual tokens, time and attempts. |

## `usage`

| Tool | Effect | What it does |
| --- | --- | --- |
| `get_usage` | Read | Reads token usage and active agent time across filtered local days. |

## `watches`

| Tool | Effect | What it does |
| --- | --- | --- |
| `get_watches` | Read | Reads which Orchestrator conversations lead which work and what waits there. |

## `worktree`

| Tool | Effect | What it does |
| --- | --- | --- |
| `remove_worktree` | Write | Removes an ended run's worktree and deletes its branch only when merged. |

## Duplication and documentation observations

- No duplicate registered names were found; separate registries and dispatch matches repeat names, schemas and descriptions, so they must stay synchronized.
- `set_allow_autopilot` is an intentional compatibility overlap with `set_approve_mode`; `get_auto_mode`, `set_auto_mode` and `set_run_auto_mode` remain listed solely to return `retired`.
- `send_message` overlaps answering questions, requesting plan changes, resuming/continuing and rerunning tasks; it selects the action from the run's state, while the dedicated tools express an explicit action.
- `create_pipeline` can copy a pipeline, overlapping `duplicate_pipeline`, which also offers profile substitution.
- `get_run`, `get_task`, `get_conversation`, `get_plan` and the waiting-item lists intentionally repeat status/plan/question information at different levels; `get_daily_report` aggregates facts also available through usage, delivery and run tools.
- `team/` provides alternate handlers for existing task, module, run and delivery operations, not additional registered tool names; wire/readiness/base-branch/merge-queue helpers likewise add no separate tools.
- Every registration has a description and input schema, including the retired tools; none is completely undocumented in MCP discovery. The existing [work manual](../manual/mcp-tools-work.md) and [setup manual](../manual/mcp-tools-setup.md) provide most of the human-facing reference.
- Those two manuals have no dedicated entries for `save_decision_connection`, `check_decision_connection`, `remove_decision_connection`, or the eight service/stack tools listed above; the setup manual only mentions decision connections through readiness fixes.
- The setup manual's draft sections lag behind current registrations: for example, `save_skill_draft` says a new skill starts empty and saving is always allowed, while the current tool creates published v1, requires a description/body and gates creation; new profiles, instructions and command tools similarly publish v1 rather than merely saving a draft.
- The setup manual's `merge_run` parameter summary lists only project/run, while its registration also describes team delivery authorization; team delivery and local merging share this name but have different authority and outcomes.

This is a static source investigation only; no builds, tests, formatting or quality-gate command were run, as requested for ORC-971.
