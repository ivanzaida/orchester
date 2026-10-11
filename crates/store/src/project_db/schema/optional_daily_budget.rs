//! ORC-9: a daily budget can be absent; existing budgets keep their value and enforcement.

pub(super) const MIGRATION: &str = r"
ALTER TABLE project_limits ADD COLUMN daily_budget_unlimited INTEGER NOT NULL DEFAULT 0
  CHECK (daily_budget_unlimited IN (0, 1));
";
