//! Drafts automation saved (ORC-30), so the person sees who wrote a draft before publishing it.

pub(super) const MIGRATION: &str = r"
-- A draft automation saved over Orchester's MCP server, and when: the draft is automation's while its `updated_at` is
-- still `saved_at`. A person's later save, or publishing it, leaves the row behind without a draft it describes.
CREATE TABLE automation_drafts (
  resource_id TEXT PRIMARY KEY REFERENCES resources (id),
  saved_at INTEGER NOT NULL
) STRICT;
";
