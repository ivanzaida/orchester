//! How each tool request was answered by its run's approval mode (`orchester_core::run::RequestAnswer`) and why one
//! came to the person (`orchester_core::run::CameToPerson`), beside who decided it. Requests answered before have
//! neither.

pub(super) const MIGRATION: &str = r"
ALTER TABLE tool_approvals ADD COLUMN answered_as TEXT;
ALTER TABLE tool_approvals ADD COLUMN came_to_person TEXT;
";
