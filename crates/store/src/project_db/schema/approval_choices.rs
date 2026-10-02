//! The approval choices of 2026-10-02: who answered each request, the rules a run keeps from its wider
//! answers, and a run's own auto-approval choice.

pub(super) const MIGRATION: &str = r"
-- Who answered a request (`DecidedBy`), set when it is claimed; NULL while it waits, and for requests
-- decided before answers were kept (a person's Approve once or Deny). The rule that answered it, if one did.
ALTER TABLE tool_approvals ADD COLUMN decided_by TEXT CHECK (decided_by IN (
  'person', 'person_similar', 'person_always', 'person_run', 'person_deny', 'similar_rule', 'always_allowed',
  'auto_approved'
));
ALTER TABLE tool_approvals ADD COLUMN rule_id INTEGER;

-- Rules a run keeps from Approve similar and Always allow (`RuleScope` as JSON). They end with the run,
-- never reach the agent's CLI and are revoked rather than deleted.
CREATE TABLE approval_rules (
  id INTEGER PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  scope TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  revoked_at INTEGER,
  answered INTEGER NOT NULL DEFAULT 0 CHECK (answered >= 0)
) STRICT;
CREATE INDEX approval_rules_run ON approval_rules (run_id);
CREATE TRIGGER approval_rules_keep_scope BEFORE UPDATE ON approval_rules
WHEN NEW.run_id IS NOT OLD.run_id OR NEW.scope IS NOT OLD.scope OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'an approval rule keeps its run and scope'); END;
CREATE TRIGGER approval_rules_no_delete BEFORE DELETE ON approval_rules
BEGIN SELECT RAISE(ABORT, 'approval rules are revoked, not deleted'); END;

-- A run's own auto-approval choice (Approve everything in this run, the run's switch, Stop); NULL
-- follows the setting of each asking profile.
ALTER TABLE runs ADD COLUMN auto_approve INTEGER CHECK (auto_approve IN (0, 1));
";
