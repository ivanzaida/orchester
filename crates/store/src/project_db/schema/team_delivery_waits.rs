//! A team delivery that waits for an Admin (the team server's S12 rule, ORC-735): with the workspace's automatic merge
//! off, a `delivery.request` that cites no authorization is no longer refused, but kept by the server as the module's
//! pending delivery, `awaiting_admin`, with no effect. Here it is `waiting`, with the delivery the server answered: it
//! goes no further under its operation, as the same operation is answered the same; once an Admin authorized it, the
//! holder's next request, under an operation of its own, takes the merge. SQLite cannot change a CHECK constraint, so
//! the table is rebuilt, its rows keeping their `rowid`, with its index and triggers made again.

pub(super) const MIGRATION: &str = r"
CREATE TABLE team_deliveries_new (
  operation_id TEXT PRIMARY KEY,
  request TEXT NOT NULL,
  stage TEXT NOT NULL CHECK (stage IN ('requested', 'refused', 'waiting', 'accepted', 'merging', 'reporting',
    'reported', 'report_refused')),
  delivery TEXT CHECK ((stage IN ('requested', 'refused')) = (delivery IS NULL)),
  refusal TEXT CHECK ((stage IN ('refused', 'report_refused')) = (refusal IS NOT NULL)),
  merge_commit TEXT,
  failure TEXT CHECK (merge_commit IS NULL OR failure IS NULL),
  report_operation_id TEXT UNIQUE
    CHECK ((stage IN ('reporting', 'reported', 'report_refused')) = (report_operation_id IS NOT NULL)),
  report_request TEXT CHECK ((report_operation_id IS NULL) = (report_request IS NULL)),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  CHECK ((report_operation_id IS NULL) = (merge_commit IS NULL AND failure IS NULL))
) STRICT;
INSERT INTO team_deliveries_new (rowid, operation_id, request, stage, delivery, refusal, merge_commit, failure,
    report_operation_id, report_request, created_at, updated_at)
  SELECT rowid, operation_id, request, stage, delivery, refusal, merge_commit, failure, report_operation_id,
    report_request, created_at, updated_at
  FROM team_deliveries;
DROP TABLE team_deliveries;
ALTER TABLE team_deliveries_new RENAME TO team_deliveries;
CREATE INDEX team_deliveries_open ON team_deliveries (created_at)
  WHERE stage IN ('requested', 'accepted', 'merging', 'reporting');
CREATE TRIGGER team_deliveries_keep_request BEFORE UPDATE ON team_deliveries
WHEN NEW.operation_id IS NOT OLD.operation_id OR NEW.request IS NOT OLD.request OR NEW.created_at IS NOT OLD.created_at
  OR (OLD.report_operation_id IS NOT NULL AND (NEW.report_operation_id IS NOT OLD.report_operation_id
    OR NEW.report_request IS NOT OLD.report_request OR NEW.merge_commit IS NOT OLD.merge_commit
    OR NEW.failure IS NOT OLD.failure))
  OR OLD.stage IN ('refused', 'waiting', 'reported', 'report_refused')
BEGIN SELECT RAISE(ABORT, 'a team delivery keeps its request and report and goes forward once'); END;
CREATE TRIGGER team_deliveries_no_delete BEFORE DELETE ON team_deliveries
BEGIN SELECT RAISE(ABORT, 'team deliveries are never deleted'); END;
";
