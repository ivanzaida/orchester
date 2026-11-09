//! A person who returns a run's work to an agent step can choose another agent profile for it, for that return only
//! (`attempts.profile`, JSON: the profile version and the model; ORC-808). The run's snapshot keeps its own agents.
//! Attempts recorded before have none.

pub(super) const MIGRATION: &str = r"
ALTER TABLE attempts ADD COLUMN profile TEXT CHECK (profile IS NULL OR reason IN ('returned', 'returned_after_end'));
";
