//! The turns of a wait that failed before the one its entry holds (`orchester_core::orchestrator::retry`,
//! `docs/plan/orchestrator-autonomy/OA-01-…`): an event whose turn failed is tried again, and its entry keeps how the
//! earlier turns ended.

pub(super) const MIGRATION: &str = r"
-- The failed turns before the entry's own, oldest first, as a JSON array; NULL while there was none.
ALTER TABLE orchestrator_entries ADD COLUMN tries TEXT;
";

/// A subscription leads its target to the end (`orchester_core::orchestrator::watch::Ceilings`,
/// `docs/plan/orchestrator-autonomy/OA-09-…`), or only follows it as every subscription did.
pub(super) const LEAD_MIGRATION: &str = r"
-- The ceilings of a subscription that leads its target to the end, as JSON; NULL for one that follows.
ALTER TABLE orchestrator_subscriptions ADD COLUMN lead TEXT;
";
