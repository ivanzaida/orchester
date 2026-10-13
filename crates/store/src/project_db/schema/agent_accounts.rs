//! Agent accounts (`docs/design/agent-accounts.md`): plan limit reports belong to the account the CLI ran on, and an
//! agent invocation records the account it started on, so a finished run still names it.

pub(super) const MIGRATION: &str = r"
-- NULL for a report from before accounts, which was the CLI's Default account's.
ALTER TABLE plan_limit_snapshots ADD COLUMN account_id TEXT;

-- The account an agent CLI invocation started on, written once before it starts. Its name and email stay in the
-- application database.
CREATE TABLE invocation_accounts (
  invocation_id TEXT PRIMARY KEY REFERENCES invocations (id),
  account_id TEXT NOT NULL
) STRICT;
CREATE TRIGGER invocation_accounts_immutable BEFORE UPDATE ON invocation_accounts
BEGIN SELECT RAISE(ABORT, 'an invocation keeps the account it started on'); END;
";
