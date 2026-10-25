//! The account an Orchestrator chat turn ran on, so its usage counts under that account (ORC-84, as the reviewer's
//! does since `reviewer_accounts`).

pub(super) const MIGRATION: &str = r"
-- The CLI account the Orchestrator's connection ran the turn on. NULL: usage recorded before the account was kept
-- (never guessed afterwards).
ALTER TABLE orchestrator_usage ADD COLUMN account_id TEXT;
";
