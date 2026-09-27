-- A project database at schema 8, before budgets were tokens, as that version wrote it: a task with a
-- budget of 5 USD and an acceptance criterion; its run of the one-step `delivery` pipeline, paused on
-- request after an agent invocation reported its usage; the run's log in event schema 1 (a money
-- budget, no time limit, a pause without a reason). Agent profiles had no permissions yet.
-- Ids end in 01 project, 02 task, 03 task batch, 04 connection, 05/06 `builder` profile/version,
-- 07/08 `delivery` pipeline/version, 09 run, 0a run idempotency key, 0b attempt, 0c step execution,
-- 0d invocation.

INSERT INTO project (id, key, name) VALUES ('01920000-0000-7000-8000-000000000001', 'ORC', 'Kontur');
UPDATE counters SET next = 2 WHERE name IN ('task', 'run');

INSERT INTO tasks (id, number, batch_key, title, brief, pipeline_id, budget_micros, budget_currency, max_duration_ms,
  status, created_at)
VALUES ('01920000-0000-7000-8000-000000000002', 1, '01920000-0000-7000-8000-000000000003', 'Say hello', '', NULL,
  5000000, 'USD', 1800000, 'draft', 1);
INSERT INTO task_criteria (task_id, position, text)
VALUES ('01920000-0000-7000-8000-000000000002', 0, 'Says hello.');

INSERT INTO resources (id, kind, slug, name, created_at) VALUES
  ('01920000-0000-7000-8000-000000000005', 'agent_profile', 'builder', 'Builder', 2),
  ('01920000-0000-7000-8000-000000000007', 'pipeline', 'delivery', 'Delivery', 4);
INSERT INTO resource_versions (id, resource_id, number, body, published_at) VALUES
  ('01920000-0000-7000-8000-000000000006', '01920000-0000-7000-8000-000000000005', 1, json('{"schema": 1, "body": {
    "role": "Builder", "responsibilities": "", "instructions": "Build.",
    "connection": "01920000-0000-7000-8000-000000000004", "model": "gpt-5.5", "settings": {}}}'), 3),
  ('01920000-0000-7000-8000-000000000008', '01920000-0000-7000-8000-000000000007', 1, json('{"schema": 1, "body": {
    "steps": [{"key": "implement", "name": "Implement",
      "kind": {"agent": {"profile": "01920000-0000-7000-8000-000000000005", "repository": "write"}},
      "instructions": "", "depends_on": [], "approval_gate": false, "on_failure": null, "budget": null}]}}'), 5);

INSERT INTO runs (id, number, idempotency_key, task_id, pipeline_version_id, snapshot, branch, base_revision, currency,
  budget_micros, spent_micros, status, status_reason, failed_step, last_sequence, created_at)
VALUES ('01920000-0000-7000-8000-000000000009', 1, '01920000-0000-7000-8000-00000000000a',
  '01920000-0000-7000-8000-000000000002', '01920000-0000-7000-8000-000000000008', json('{"schema": 1, "body": {
    "pipeline": {"resource": "01920000-0000-7000-8000-000000000007", "version": "01920000-0000-7000-8000-000000000008",
      "number": 1},
    "body": {"steps": [{"key": "implement", "name": "Implement",
      "kind": {"agent": {"profile": "01920000-0000-7000-8000-000000000005", "repository": "write"}},
      "instructions": "", "depends_on": [], "approval_gate": false, "on_failure": null, "budget": null}]},
    "steps": [{"key": "implement", "executor": {"type": "agent", "model": "gpt-5.5", "model_overridden": false,
      "profile": {"resource": "01920000-0000-7000-8000-000000000005", "version": "01920000-0000-7000-8000-000000000006",
        "number": 1}}}],
    "instructions": [], "tools": []}}'),
  'orchester/orc-1-run-1', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 'USD', 5000000, 0, 'paused', NULL, NULL, 4, 6);
INSERT INTO attempts (id, run_id, number, reason, failed_step, return_to, status)
VALUES ('01920000-0000-7000-8000-00000000000b', '01920000-0000-7000-8000-000000000009', 1, 'initial', NULL, NULL,
  'active');
INSERT INTO step_executions (id, attempt_id, step_key, profile_version_id, model, status, wait_reason)
VALUES ('01920000-0000-7000-8000-00000000000c', '01920000-0000-7000-8000-00000000000b', 'implement',
  '01920000-0000-7000-8000-000000000006', 'gpt-5.5', 'pending', NULL);

INSERT INTO run_events (run_id, sequence, at, kind, payload) VALUES
  ('01920000-0000-7000-8000-000000000009', 1, 6, 'created', json('{"schema": 1, "body": {"kind": "created",
    "task": "01920000-0000-7000-8000-000000000002", "number": 1,
    "snapshot": {
      "pipeline": {"resource": "01920000-0000-7000-8000-000000000007",
        "version": "01920000-0000-7000-8000-000000000008", "number": 1},
      "body": {"steps": [{"key": "implement", "name": "Implement",
        "kind": {"agent": {"profile": "01920000-0000-7000-8000-000000000005", "repository": "write"}},
        "instructions": "", "depends_on": [], "approval_gate": false, "on_failure": null, "budget": null}]},
      "steps": [{"key": "implement", "executor": {"type": "agent", "model": "gpt-5.5", "model_overridden": false,
        "profile": {"resource": "01920000-0000-7000-8000-000000000005",
          "version": "01920000-0000-7000-8000-000000000006", "number": 1}}}],
      "instructions": [], "tools": []},
    "branch": "orchester/orc-1-run-1", "base": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "budget": {"micros": 5000000, "currency": "USD"}}}')),
  ('01920000-0000-7000-8000-000000000009', 2, 7, 'started', json('{"schema": 1, "body": {"kind": "started"}}')),
  ('01920000-0000-7000-8000-000000000009', 3, 7, 'attempt_started', json('{"schema": 1, "body": {
    "kind": "attempt_started", "attempt": "01920000-0000-7000-8000-00000000000b", "number": 1, "reason": "initial",
    "executions": [{"id": "01920000-0000-7000-8000-00000000000c", "step": "implement"}]}}')),
  ('01920000-0000-7000-8000-000000000009', 4, 20, 'paused', json('{"schema": 1, "body": {"kind": "paused"}}'));

INSERT INTO invocations (id, run_id, step_execution_id, number, model, session_id, started_at, ended_at, input_tokens,
  cached_input_tokens, output_tokens, outcome)
VALUES ('01920000-0000-7000-8000-00000000000d', '01920000-0000-7000-8000-000000000009',
  '01920000-0000-7000-8000-00000000000c', 1, 'gpt-5.5', NULL, 10, NULL, 12166, 2432, 5, NULL);
