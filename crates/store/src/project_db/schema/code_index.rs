//! The code index (ORC-38): the project's setting and what each agent invocation got of the index
//! and used before its first edit.

pub(super) const MIGRATION: &str = r"
-- Project settings › Code index › Use code index; without a row the project has it on.
CREATE TABLE project_code_index (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  setting TEXT NOT NULL CHECK (setting IN ('on', 'off'))
) STRICT;

-- What each agent invocation got of the code index, written once before its CLI starts.
CREATE TABLE invocation_code_index (
  invocation_id TEXT PRIMARY KEY REFERENCES invocations (id),
  body TEXT NOT NULL
) STRICT;

-- The tokens each agent invocation used before its first file edit, and that edit; rewritten
-- while the invocation runs.
CREATE TABLE invocation_edit_progress (
  invocation_id TEXT PRIMARY KEY REFERENCES invocations (id),
  body TEXT NOT NULL
) STRICT;
";
