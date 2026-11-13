# Project database migrations — 2026-11-13

Source: [schema/list.rs](../../crates/store/src/project_db/schema/list.rs) and every module it names, inspected at run base `2ab1600da304e86818a821d9340406e8885bc1a4`.
There are **170 migrations**; the numbers below are their positions in `MIGRATIONS`, not task numbers or module declaration order.
Module names below refer to files beside `list.rs`; inline entries are SQL strings in that file.
This is the project database, not the application database.

## Execution order

1. `initial::PROJECT` — Creates the project, task counter, tasks and ordered task criteria.
2. `initial::RESOURCES` — Creates resources, drafts and immutable published versions.
3. `initial::RUNS` — Creates runs, attempts, step executions, immutable checkpoints and append-only run events, and seeds the run counter.
4. `list (inline)` — Creates named connections with settings and check results.
5. `list (inline)` — Creates the singleton GitHub repository link.
6. `list (inline)` — Creates repository sync timestamps and cached pull requests.
7. `list (inline)` — Creates blob metadata and per-head pull-request revision diffs.
8. `list (inline)` — Creates CLI invocations and append-only agent transcript events.
9. `token_budgets::MIGRATION` — Converts task and run money budgets to tokens while preserving old amounts and adds project limits.
10. `list (inline)` — Creates immutable step outputs, execution failure reasons and an archive of former pipeline step money budgets.
11. `list (inline)` — Adds the run's limiting execution and per-run step token-budget overrides.
12. `list (inline)` — Creates per-run agent-profile permission overrides.
13. `list (inline)` — Creates cached commit-to-commit revision diffs.
14. `list (inline)` — Creates the saved prompt body for each invocation.
15. `list (inline)` — Creates project secret metadata with unique active names per environment.
16. `list (inline)` — Adds command invocations and rebuilds step executions to admit command failures.
17. `list (inline)` — Creates immutable, once-decided numbered step plans.
18. `list (inline)` — Creates case-insensitive project environment variables.
19. `list (inline)` — Adds runs' base branches and records their merges.
20. `conversation::MIGRATION` — Creates task conversation messages and permits follow-up attempts.
21. `import::MIGRATION` — Creates resource import provenance and retry keys.
22. `mcp::MIGRATION` — Creates project MCP server settings.
23. `agent_files::MIGRATION` — Adds the skill resource kind, repository agent-file settings and repository import provenance.
24. `task_pipeline_pin::MIGRATION` — Adds the selected pipeline version pin to tasks.
25. `task_edits::MIGRATION` — Creates immutable task-content snapshots for runs.
26. `tool_approvals::ACTIVE_TIME_MIGRATION` — Adds invocation inactive time and the current wait start.
27. `tool_approvals::MIGRATION` — Creates single-use tool approval requests.
28. `recovery::MIGRATION` — Permits recovery attempts and adds message recovery sequences.
29. `command_approvals::MIGRATION` — Adds command approval metadata and approval interruption records.
30. `invocation_permissions::MIGRATION` — Adds immutable invocation permission snapshots.
31. `code_index::MIGRATION` — Creates the code-index setting, invocation index snapshots and first-edit progress.
32. `memory::MIGRATION` — Creates memories with FTS indexes, memory settings and invocation memory selections.
33. `native_commands::MIGRATION` — Adds native command shell and time-limit metadata.
34. `task_groups::MIGRATION` — Creates reviewed task groups and ordered membership.
35. `task_groups::FINISHED_MIGRATION` — Adds a finished flag to task groups.
36. `stuck_runs::MIGRATION` — Rebuilds runs and attempts for stuck-run waits, restarts and returns.
37. `steering::MIGRATION` — Adds message timing, arrival and undelivered-message metadata.
38. `approval_choices::MIGRATION` — Adds approval decision attribution, retained approval rules and per-run auto-approval choice.
39. `organization::MIGRATION` — Creates task organization, project tags and organization history, deriving existing task statuses from runs.
40. `worktree_removals::MIGRATION` — Records successful run worktree removals.
41. `sharing::MIGRATION` — Adds shared task revisions and attribution plus server links, claims, run statuses and automatic status changes.
42. `joining::MIGRATION` — Rebuilds tasks for retained nonshared identities and creates joined-key mappings.
43. `permitted_decisions::MIGRATION` — Allows tool approvals decided by permissions.
44. `automation_answers::MIGRATION` — Allows automation approval and denial attribution.
45. `auto_mode::MIGRATION` — Creates project and run auto-mode settings and reviewer decisions on requests.
46. `auto_mode::REVIEWER_MIGRATION` — Adds the reviewer's connection and effort selection.
47. `auto_mode::ORIGIN_MIGRATION` — Adds each run's starting origin push URL.
48. `auto_mode::REVIEWING_MIGRATION` — Adds the request reviewing flag.
49. `native_conversations::MIGRATION` — Creates native conversations, append-only items and invocation engine snapshots.
50. `agent_slots::MIGRATION` — Adds the concurrent agent-slot limit, defaulting to four.
51. `plan_limits::MIGRATION` — Creates time-indexed CLI plan-limit snapshots.
52. `reviewer_usage::MIGRATION` — Creates reviewer usage and verdict records.
53. `questions::MIGRATION` — Creates agent question requests with answers and interruption state.
54. `plan_decided_by::MIGRATION` — Adds plan decision authors and backfills existing decisions as the person's.
55. `sources::MIGRATION` — Creates ordered project and task external read sources.
56. `step_progress::MIGRATION` — Creates each execution's latest progress report.
57. `optional_limits::MIGRATION` — Makes task, run and step limits optional and removes the maximum run budget while retaining values.
58. `optional_daily_budget::MIGRATION` — Adds an unlimited-daily-budget flag.
59. `agent_accounts::MIGRATION` — Adds invocation accounts and step connection overrides and permits runs paused at plan limits.
60. `orchestrator::MIGRATION` — Creates versioned Orchestrator settings, chats, entries and usage.
61. `queue_holds::MIGRATION` — Creates project and group queue holds.
62. `unverified_steps::MIGRATION` — Rebuilds attempts, executions and outputs for accepted, unverified and unchanged results with judged revisions.
63. `local_integration::MIGRATION` — Creates local integration state, recovery file content and process progress.
64. `editor_drafts::MIGRATION` — Rebuilds recovery content to retain unfinished editor drafts.
65. `sharing::ADDRESS_MIGRATION` — Adds the shared project's address.
66. `pull_request_syncs::MIGRATION` — Adds pull-request merge/close times, fork status and sync attempt results.
67. `worktree_not_created::MIGRATION` — Rebuilds runs to record typed worktree-creation failures without a failed step.
68. `shared_members::MIGRATION` — Creates cached shared members and repository/member read timestamps.
69. `shared_members::READ_AS_MIGRATION` — Adds the identity used to read shared membership.
70. `orchestrator_waits::MIGRATION` — Rebuilds Orchestrator entries for agent waits and adds Orchestrator message/question attribution.
71. `run_rebases::MIGRATION` — Creates each run's latest rebase record.
72. `creation_snapshots::MIGRATION` — Creates immutable task creation snapshots.
73. `api_providers::MIGRATION` — Adds connection API-provider settings and invocation provider snapshots.
74. `project_approval_rules::MIGRATION` — Creates persistent project approval rules.
75. `decision_authors::MIGRATION` — Adds automation attribution to messages and backfills Orchestrator messages.
76. `returned_after_end::MIGRATION` — Allows returned-after-end attempts with conflict details.
77. `run_resolutions::MIGRATION` — Creates pending run merge conflict resolutions.
78. `orchestrator_run_ends::MIGRATION` — Creates and seeds the Orchestrator's run-end following start time.
79. `base_notices::MIGRATION` — Creates observed base heads, pending base-move notices and the project notice switch.
80. `orchestrator_sent::MIGRATION` — Adds the source Orchestrator chat to messages and questions.
81. `qa_slots::MIGRATION` — Adds the concurrent QA-app limit, defaulting to two.
82. `orchestrator_reports::MIGRATION` — Rebuilds Orchestrator entries to admit daily reports.
83. `modules::MIGRATION` — Creates modules, task membership and module change history.
84. `runner_failures::MIGRATION` — Creates typed runner interruption failure records.
85. `story_points::MIGRATION` — Creates task point estimates, estimator usage and merged task effort.
86. `attachments::MIGRATION` — Creates ordered message attachment metadata.
87. `reviewer_accounts::MIGRATION` — Adds reviewer usage account attribution.
88. `denial_notes::MIGRATION` — Adds immutable denial notes to tool requests.
89. `automation_drafts::MIGRATION` — Creates automation draft authorship marks.
90. `feedback::MIGRATION` — Creates pull-request feedback threads and their posts.
91. `modules::SHARED_MIGRATION` — Adds shared module revisions and server change sequences.
92. `import::RECORD_MIGRATION` — Adds imported resource names and ordered unresolved/replaced import results.
93. `merge_queue::MIGRATION` — Creates merge queue state and events and permits squash merge records.
94. `base_branch::MIGRATION` — Creates the project default base-branch setting.
95. `orchestrator_accounts::MIGRATION` — Adds account attribution to Orchestrator usage.
96. `orchestrator_queue::MIGRATION` — Rebuilds Orchestrator entries for queued and mid-turn messages.
97. `orchestrator_senders::MIGRATION` — Adds automation attribution to Orchestrator messages.
98. `run_completions::MIGRATION` — Creates run completions without merges and named run artifacts.
99. `story_points::SETTING_MIGRATION` — Creates the project's estimate setting.
100. `default_pipeline::MIGRATION` — Creates the project default pipeline setting.
101. `removed_resources::MIGRATION` — Creates removed-resource archive marks.
102. `usage_providers::MIGRATION` — Creates invocation connection snapshots and adds reviewer connection attribution.
103. `workspaces::MIGRATION` — Creates reusable workspaces, borrowing runs, workspace waits, settings, warm folders and warm-up failures.
104. `approve_everything::MIGRATION` — Creates the project approve-everything setting.
105. `base_notice_merges::MIGRATION` — Rebuilds base notices to record runner-performed merges without invocations.
106. `test_suites::MIGRATION` — Adds the test-suite resource kind.
107. `run_completions::MEMBER_MIGRATION` — Permits run completion by a team member.
108. `services::MIGRATION` — Adds service and stack resource kinds.
109. `approval_modes::MIGRATION` — Replaces old auto-mode switches with project Autopilot permission and per-run Manual overrides.
110. `shared_configuration::MIGRATION` — Adds shared resource revisions, publisher metadata, version bodies/references and local reference bindings.
111. `request_answers::MIGRATION` — Adds approval-mode answer and came-to-person explanations.
112. `approval_modes::SWITCH_MIGRATION` — Adds when and by whom a run switched to Manual.
113. `shared_publishes::MIGRATION` — Creates pending shared publications and adds the server-admin flag.
114. `waiting_versions::MIGRATION` — Creates waiting shared-version acceptance marks.
115. `base_sources::MIGRATION` — Moves project base branches to open modules, adds task/run base provenance and drops the project base-branch table.
116. `shared_members::SERVER_ADMINS_MIGRATION` — Creates cached server-admin identities.
117. `shared_publishes::REOPEN_MIGRATION` — Adjusts version immutability to allow taking back unused unnumbered shared publications.
118. `worktree_removals::NOT_REMOVED_MIGRATION` — Creates records of incomplete worktree removals.
119. `disk_guard::MIGRATION` — Separates disk guard settings from workspace settings, preserving free-space values only when workspaces were enabled.
120. `orchestrator_switches::MIGRATION` — Creates per-project Orchestrator capability switches.
121. `orchestrator_todos::MIGRATION` — Creates the latest todo list for each Orchestrator turn.
122. `context_use::MIGRATION` — Creates invocation context readings and adds chat session context readings.
123. `permission_events::MIGRATION` — Creates append-only run permission change events.
124. `orchestrator_titles::MIGRATION` — Adds person-chosen chat titles and pin timestamps.
125. `run_models::MIGRATION` — Creates per-run step model overrides and their event history.
126. `orchestrator_archive::MIGRATION` — Adds chat archive timestamps and detaches usage foreign keys so usage survives chat deletion.
127. `plan_only::MIGRATION` — Adds chat Plan only settings and per-message planning modes.
128. `side_questions::MIGRATION` — Creates side questions with answers, failures and send-once state.
129. `ci_fixes::MIGRATION` — Creates run CI-fix settings and append-only events.
130. `merge_attempts::MIGRATION` — Creates completed and open merge-attempt records.
131. `review_threads::MIGRATION` — Creates run review threads, ordered messages and per-head Viewed marks.
132. `merge_attempts::CAUSE_KIND_MIGRATION` — Adds merge-attempt cause kinds.
133. `archive_stops::MIGRATION` — Creates stops caused by archived team resources.
134. `decision_connections::MIGRATION` — Creates decision connections and the selected decision reviewer connection.
135. `team_reviewer::MIGRATION` — Adds the team reviewer resource kind.
136. `decision_reviewer::MIGRATION` — Adds decision reviewer settings.
137. `decision_review::MIGRATION` — Adds decision model/threshold selection and detailed reviewer failure metadata.
138. `decision_tests::MIGRATION` — Creates explicit decision-model comparison records.
139. `task_identities::MIGRATION` — Separates durable task identities from definitions and local base overrides, retargeting historical foreign keys.
140. `run_inputs::MIGRATION` — Creates immutable run bundles, local bindings and local consent.
141. `run_inputs::ACCEPTED_MIGRATION` — Creates immutable accepted inputs for not-yet-started team runs.
142. `team_operations::MIGRATION` — Creates durable idempotent team operations with requests and receipts.
143. `team_work::MIGRATION` — Creates resumable team starts, team run authority records and the work-report outbox.
144. `team_delivery::MIGRATION` — Creates resumable module delivery operations and reports.
145. `team_delivery_waits::MIGRATION` — Rebuilds team deliveries to permit waiting for Admin authorization.
146. `reported_windows::MIGRATION` — Adds the chat session's last reported context window and model.
147. `team_pipeline_ids::MIGRATION` — Converts cached shared task pipeline slugs to team resource ids while retaining the column name.
148. `orchestrator_watch::MIGRATION` — Creates project Orchestrator watch settings.
149. `run_waits_on::MIGRATION` — Creates run-to-task dependency waits and their ended history.
150. `orchestrator_subscriptions::MIGRATION` — Creates Orchestrator target subscriptions with exclusive active leadership.
151. `run_waits_on::ANSWERED_MIGRATION` — Rebuilds run dependency waits to permit the answered ending.
152. `run_waits_on::DROPPED_MIGRATION` — Rebuilds dependency waits to permit dropping waits, including eligible ended waits.
153. `chosen_profiles::MIGRATION` — Adds a chosen agent profile for returned attempts.
154. `agent_event_kinds::MIGRATION` — Indexes agent events by run, kind and sequence.
155. `orchestrator_switches::DRIVE_MIGRATION` — Adds the single drives switch, copying answers_agents while leaving old switches unread.
156. `team_fill_ins::MIGRATION` — Creates local team connection/account picks, MCP switches and secret keychain handles.
157. `team_mcp_server::MIGRATION` — Adds the team MCP-server resource kind.
158. `team_references::MIGRATION` — Rebuilds shared references for team ids, CLI kinds and secret names, converting tool references to team_tool.
159. `ci_fixes::NOT_READY_MIGRATION` — Rebuilds CI-fix events to permit not-ready messages without counts or limits.
160. `created_here::MIGRATION` — Marks publications whose resource was created directly here.
161. `team_start_notes::MIGRATION` — Adds runner refusal details, seen marks and a newest-start index.
162. `archive_stops::ARCHIVED_MIGRATION` — Adds archived resource identity, kind and author to archive stops.
163. `remove_bindings::MIGRATION` — Drops reference bindings and obsolete reference kinds and removes resource slug uniqueness across local/team rows.
164. `local_archive::MIGRATION` — Creates local MCP archive marks and MCP archive stop records.
165. `profile_given::MIGRATION` — Rewrites local profile drafts and published bodies with explicit instruction, skill and tool selections.
166. `team_outbox::MIGRATION` — Drops publication outbox and waiting-version tables, removes unpinned unsent versions and shared drafts, and restores version guards.
167. `local_drafts::MIGRATION` — Deletes drafts of already-published local profiles, instructions, skills and tools.
168. `approval_modes::DANGEROUS_MIGRATION` — Adds Dangerous auto-approve to project approval settings, defaulting off.
169. `pipeline_drafts::MIGRATION` — Deletes all pipeline drafts and removes unpublished pipeline resources and dependent marks.
170. `team_cache::MIGRATION` — Removes unpinned shared resource versions and their cache rows, then removes team resources left without versions.

## Last three: exact effects

### 168 — approval_modes::DANGEROUS_MIGRATION

Adds `project_approvals.dangerous_auto_approve INTEGER NOT NULL DEFAULT 0`.
Its CHECK permits only 0 or 1 and requires `allow_autopilot = 1` whenever the new flag is 1.
Existing settings rows therefore retain Manual or Autopilot with Dangerous auto-approve off.
It creates no table, inserts no settings row and deletes nothing; projects without a row still have no row.

### 169 — pipeline_drafts::MIGRATION

Collects pipeline resources without any published version in temporary `dropped_pipelines`.
For tasks naming those ids it sets both `pipeline_id` and `pipeline_version` to NULL.
It deletes their `project_default_pipeline`, `resource_imports`, `repository_imports` and `removed_resources` rows, then their `resources` rows.
For **every** pipeline resource, published or not, it deletes `resource_drafts`, `automation_drafts` marks and `money_step_budgets_before_tokens` rows whose owner is `draft`.
It drops only the temporary table, not permanent tables.
Published pipeline versions, task creation snapshots and shared tasks' `pipeline_slug` remain unchanged; a retained reference to a removed pipeline can consequently read as missing.

### 170 — team_cache::MIGRATION

Only resources with `sharing IS 'shared'` in a database containing a `sharing` row are candidates.
Temporary tables collect candidates, pinned versions, versions to delete and resources left without versions.
A version stays if a run names its id as `pipeline_version_id`, its id occurs anywhere in a run's snapshot text (`instr`), or a task names its resource id and version number.
For all other candidate versions it deletes `shared_version_references`, `shared_version_bodies` and `resource_versions` rows, temporarily dropping and then restoring `resource_versions_no_delete`.
For candidate resources with no remaining versions it clears tasks' `pipeline_id` (this SQL does **not** clear `pipeline_version`), deletes their `project_default_pipeline`, `resource_drafts`, `removed_resources`, `repository_imports`, `automation_drafts` and `resource_imports` rows, then deletes the `resources` rows.
Pinned versions retain their resource, bodies and references.
Local resources and computer-specific fill-ins, connection picks, MCP switches, secret handles and program paths remain.
All four temporary tables are dropped; no permanent table is added or dropped.

## Ordering and documentation observations

- The list explicitly requires append-only order; no dependency reversal was apparent in the inspected SQL.
  Repeated module names are deliberate later migrations, not duplicates: for example `modules::SHARED_MIGRATION` is declared before `modules::MIGRATION` in its file, but executes after the latter creates the tables.
- File order, task numbers and dates are not execution order: the ORC-593 switches migration precedes the ORC-304 context migration, and the later ORC-909 drive migration follows ORC-914's index.
  Reordering by those labels would misrepresent the list.
- Several early inline entries (connections, repository caches, invocations, etc.) have no individual symbolic migration name; connections also has no explanatory SQL comment.
  `local_integration` and `decision_reviewer` have no prose explanation, and `decision_connections` has only a short separation comment.
  The descriptions above for these entries come from their SQL.
- Historical documentation describes intermediate states: `local_drafts` says pipeline drafts remain, but migration 169 removes them; `shared_configuration` introduces bindings that 163 drops, and publication/waiting tables introduced at 113/114 disappear at 166.
  These are later changes, not reasons to rewrite earlier migrations.
- The team-cache header broadly says the shared database keeps no team configuration, but its documented pin exception and SQL retain historical versions and resources.
  Its snapshot pin test is a textual substring test rather than a parsed reference lookup.
- Migration 169 clears both task pipeline columns; migration 170 clears only the id.
  This difference is recorded here without asserting a defect or changing code.

## Validation scope

Read-only source investigation followed by this note only; no Rust or visual changes.
Builds, tests, formatting and `tools/check.ps1` were not run, following ORC-970's specific “Do not run builds or tests” scope (the general gate runs build-based checks).
