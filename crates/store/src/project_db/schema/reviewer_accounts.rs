//! The account auto mode's reviewer ran on, so its usage counts under that account (decided 2026-10-22).

pub(super) const MIGRATION: &str = r"
-- The CLI account the reviewer's connection ran on when the review ran. NULL: a native reviewer, a provider
-- without accounts, or a review recorded before the account was kept (never guessed afterwards).
ALTER TABLE reviewer_usage ADD COLUMN account_id TEXT;
";
