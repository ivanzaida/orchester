//! A local project's agent profiles, instructions, skills and tools are edited in place with no drafts (decided by the
//! person 2026-11-09): the Agents hub's Save publishes the panel's fields as the next version at once. The drafts the
//! old pages and their autosave stored of those resources go, so none opens a panel as changes the person never made.
//! A resource that was never published keeps its draft, which is all of it. A team project's are not touched, nor a
//! pipeline's drafts (a local pipeline is still edited through its draft), nor anything a task or a run uses: they pin
//! published versions, never a draft. Running it again changes nothing.

pub(super) const MIGRATION: &str = r"
DELETE FROM resource_drafts
  WHERE NOT EXISTS (SELECT 1 FROM sharing)
  AND resource_id IN (SELECT id FROM resources WHERE kind IN ('agent_profile', 'instruction', 'skill', 'tool'))
  AND EXISTS (SELECT 1 FROM resource_versions WHERE resource_versions.resource_id = resource_drafts.resource_id);
";
