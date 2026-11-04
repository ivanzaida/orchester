//! Team work started through the server and its reports (team authority P4, `docs/cloud-api.md`, "Team authority P4
//! work"). A start is recorded by its `work.start` operation before the command is sent, with what its caller read and
//! chose here (`request`), and goes forward one boundary at a time: the receipt's work and bundle reference kept
//! (`accepted`), the bundle's inputs and files kept with the run's accepted inputs in one transaction
//! (`inputs_stored`), the runner's run recorded (`started`), or the server's refusal (`refused`). So a crash at any
//! boundary is carried on from it, once.
//!
//! A started team run keeps the authority it reports under, its server acceptance apart from its local status (the
//! work as the server last answered one of its reports, by revision), the last outcome it queued, and its fence: once
//! the server says the authority is gone, the run reports nothing more and its output stays here. Its reports wait in
//! the outbox, each made once with its event id, per-run sequence, operation id and request, and sent in sequence
//! order until the server acknowledges or refuses it. Nothing here is a secret, and no record is deleted.

pub(super) const MIGRATION: &str = r"
CREATE TABLE team_starts (
  operation_id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES task_identities (id),
  request TEXT NOT NULL,
  stage TEXT NOT NULL CHECK (stage IN ('requested', 'accepted', 'inputs_stored', 'started', 'refused')),
  work_id TEXT CHECK (stage = 'refused' OR (stage = 'requested') = (work_id IS NULL)),
  bundle_sha256 TEXT CHECK ((work_id IS NULL) = (bundle_sha256 IS NULL) AND length(bundle_sha256) = 64),
  run_id TEXT CHECK ((stage = 'started') = (run_id IS NOT NULL)),
  refusal TEXT CHECK ((stage = 'refused') = (refusal IS NOT NULL)),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
) STRICT;
CREATE INDEX team_starts_open ON team_starts (created_at) WHERE stage IN ('requested', 'accepted', 'inputs_stored');
CREATE TRIGGER team_starts_keep_request BEFORE UPDATE ON team_starts
WHEN NEW.operation_id IS NOT OLD.operation_id OR NEW.task_id IS NOT OLD.task_id OR NEW.request IS NOT OLD.request
  OR NEW.created_at IS NOT OLD.created_at
  OR (OLD.work_id IS NOT NULL AND (NEW.work_id IS NOT OLD.work_id OR NEW.bundle_sha256 IS NOT OLD.bundle_sha256))
  OR OLD.stage IN ('started', 'refused')
BEGIN SELECT RAISE(ABORT, 'a team start keeps its request and goes forward once'); END;
CREATE TRIGGER team_starts_no_delete BEFORE DELETE ON team_starts
BEGIN SELECT RAISE(ABORT, 'team starts are never deleted'); END;

CREATE TABLE team_runs (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  operation_id TEXT NOT NULL UNIQUE REFERENCES team_starts (operation_id),
  team_task_id TEXT NOT NULL,
  claim_id TEXT NOT NULL,
  work_id TEXT NOT NULL,
  next_sequence INTEGER NOT NULL CHECK (next_sequence > 0),
  last_outcome TEXT,
  accepted_work TEXT,
  accepted_revision INTEGER CHECK ((accepted_work IS NULL) = (accepted_revision IS NULL) AND accepted_revision > 0),
  fence TEXT CHECK (fence IN ('claim_lost', 'membership_revoked', 'session_mismatch', 'superseded')),
  fenced_at INTEGER CHECK ((fence IS NULL) = (fenced_at IS NULL))
) STRICT;
CREATE TRIGGER team_runs_keep_authority BEFORE UPDATE ON team_runs
WHEN NEW.run_id IS NOT OLD.run_id OR NEW.operation_id IS NOT OLD.operation_id OR NEW.team_task_id IS NOT OLD.team_task_id
  OR NEW.claim_id IS NOT OLD.claim_id OR NEW.work_id IS NOT OLD.work_id OR NEW.next_sequence < OLD.next_sequence
  OR (OLD.fence IS NOT NULL AND (NEW.fence IS NOT OLD.fence OR NEW.fenced_at IS NOT OLD.fenced_at))
BEGIN SELECT RAISE(ABORT, 'a team run keeps its authority and its fence'); END;
CREATE TRIGGER team_runs_no_delete BEFORE DELETE ON team_runs
BEGIN SELECT RAISE(ABORT, 'team runs are never deleted'); END;

CREATE TABLE work_report_outbox (
  event_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES team_runs (run_id),
  sequence INTEGER NOT NULL CHECK (sequence > 0),
  operation_id TEXT NOT NULL UNIQUE,
  outcome TEXT NOT NULL,
  request TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('pending', 'acknowledged', 'refused')),
  refusal TEXT CHECK ((state = 'refused') = (refusal IS NOT NULL)),
  created_at INTEGER NOT NULL,
  settled_at INTEGER CHECK ((state = 'pending') = (settled_at IS NULL)),
  UNIQUE (run_id, sequence)
) STRICT;
CREATE TRIGGER work_report_outbox_keep_report BEFORE UPDATE ON work_report_outbox
WHEN NEW.event_id IS NOT OLD.event_id OR NEW.run_id IS NOT OLD.run_id OR NEW.sequence IS NOT OLD.sequence
  OR NEW.operation_id IS NOT OLD.operation_id OR NEW.outcome IS NOT OLD.outcome OR NEW.request IS NOT OLD.request
  OR NEW.created_at IS NOT OLD.created_at OR OLD.state <> 'pending'
BEGIN SELECT RAISE(ABORT, 'a work report keeps its request and is settled once'); END;
CREATE TRIGGER work_report_outbox_no_delete BEFORE DELETE ON work_report_outbox
BEGIN SELECT RAISE(ABORT, 'work reports are never deleted'); END;
";
