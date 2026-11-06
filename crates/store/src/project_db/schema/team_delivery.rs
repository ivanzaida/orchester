//! Team module deliveries (team authority P4c, `docs/cloud-api.md`, "Team authority P4 module delivery"): a
//! `delivery.request` of a module's branch, recorded by its operation before the command is sent, with what its caller
//! read here (`request`), and carried forward one boundary at a time: the server's refusal (`refused`), the receipt's
//! delivery with its `deliver_module` effect (`accepted`), the local merge begun (`merging`), its outcome with the
//! `delivery.report` made for it (`reporting`), and the server's answer to that report (`reported`, or
//! `report_refused`). The report is made once, with its own operation id and request, in the same transaction as the
//! merge's outcome, and sent again with the same bytes until the server answers it. `delivery` is the delivery as the
//! server last answered it. Nothing here is a secret, and no record is deleted.

pub(super) const MIGRATION: &str = r"
CREATE TABLE team_deliveries (
  operation_id TEXT PRIMARY KEY,
  request TEXT NOT NULL,
  stage TEXT NOT NULL CHECK (stage IN ('requested', 'refused', 'accepted', 'merging', 'reporting', 'reported',
    'report_refused')),
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
CREATE INDEX team_deliveries_open ON team_deliveries (created_at)
  WHERE stage IN ('requested', 'accepted', 'merging', 'reporting');
CREATE TRIGGER team_deliveries_keep_request BEFORE UPDATE ON team_deliveries
WHEN NEW.operation_id IS NOT OLD.operation_id OR NEW.request IS NOT OLD.request OR NEW.created_at IS NOT OLD.created_at
  OR (OLD.report_operation_id IS NOT NULL AND (NEW.report_operation_id IS NOT OLD.report_operation_id
    OR NEW.report_request IS NOT OLD.report_request OR NEW.merge_commit IS NOT OLD.merge_commit
    OR NEW.failure IS NOT OLD.failure))
  OR OLD.stage IN ('refused', 'reported', 'report_refused')
BEGIN SELECT RAISE(ABORT, 'a team delivery keeps its request and report and goes forward once'); END;
CREATE TRIGGER team_deliveries_no_delete BEFORE DELETE ON team_deliveries
BEGIN SELECT RAISE(ABORT, 'team deliveries are never deleted'); END;
";
