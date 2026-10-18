//! Rules of the project (`orchester_core::run::ProjectRule`, the person's decision of 2026-10-18): *Approve for every
//! run of this project* on a `start_qa_app` request keeps one, which answers the tool in every run while the QA
//! script is the one approved (`RuleScope` as JSON, as `approval_rules` keeps it). The run and the request a person
//! approved with it (`made_by`, the request's id) are kept for the list and the card's mark; rules are revoked rather
//! than deleted, and stay revoked.

pub(super) const MIGRATION: &str = r"
CREATE TABLE project_approval_rules (
  id INTEGER PRIMARY KEY,
  scope TEXT NOT NULL,
  made_in TEXT NOT NULL REFERENCES runs (id),
  made_by TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  revoked_at INTEGER,
  answered INTEGER NOT NULL DEFAULT 0 CHECK (answered >= 0)
) STRICT;
CREATE TRIGGER project_approval_rules_keep_scope BEFORE UPDATE ON project_approval_rules
WHEN NEW.scope IS NOT OLD.scope OR NEW.made_in IS NOT OLD.made_in OR NEW.made_by IS NOT OLD.made_by
  OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'a project approval rule keeps its scope'); END;
CREATE TRIGGER project_approval_rules_stay_revoked BEFORE UPDATE ON project_approval_rules
WHEN OLD.revoked_at IS NOT NULL AND NEW.revoked_at IS NOT OLD.revoked_at
BEGIN SELECT RAISE(ABORT, 'a revoked project approval rule stays revoked'); END;
CREATE TRIGGER project_approval_rules_no_delete BEFORE DELETE ON project_approval_rules
BEGIN SELECT RAISE(ABORT, 'project approval rules are revoked, not deleted'); END;
";
