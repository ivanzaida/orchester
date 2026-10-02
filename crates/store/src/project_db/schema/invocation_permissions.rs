//! Enforced launch policies are immutable; older invocations have no recoverable snapshot.

pub(super) const MIGRATION: &str = r"
ALTER TABLE invocations ADD COLUMN permissions TEXT CHECK (permissions IS NULL OR kind = 'agent');
CREATE TRIGGER invocations_permissions_immutable BEFORE UPDATE OF permissions ON invocations
WHEN OLD.permissions IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'invocation permissions are immutable'); END;
";
