//! Task and project memory (Memory 1/3): the records, their full-text indexes, the project's
//! memory setting and what each invocation was given.

pub(super) const MIGRATION: &str = r"
-- One memory record per row: the task's (task_id set) or the project's (task_id NULL). `seq` is the
-- rowid the full-text indexes point at; an explicit INTEGER PRIMARY KEY, so VACUUM keeps it. An
-- agent's record names its profile, run, step and execution; a person's names none. `tags` holds the
-- record's tags separated by spaces, `source_events` a JSON array of the run transcript sequences it
-- came from, `edited_at` when a person last edited it. A key names one active record of its scope.
CREATE TABLE memories (
  seq INTEGER PRIMARY KEY,
  id TEXT NOT NULL UNIQUE,
  task_id TEXT REFERENCES tasks (id),
  kind TEXT NOT NULL CHECK (kind IN ('fact', 'decision', 'observation', 'artifact', 'handoff', 'todo', 'warning',
    'summary')),
  key TEXT,
  text TEXT NOT NULL CHECK (trim(text) <> ''),
  tags TEXT NOT NULL,
  importance TEXT NOT NULL CHECK (importance IN ('low', 'normal', 'high')),
  confidence TEXT NOT NULL CHECK (confidence IN ('low', 'medium', 'high')),
  status TEXT NOT NULL CHECK (status IN ('proposed', 'active', 'superseded', 'deprecated', 'rejected')),
  pinned INTEGER NOT NULL CHECK (pinned IN (0, 1)),
  supersedes_id TEXT REFERENCES memories (id) ON DELETE SET NULL,
  author TEXT NOT NULL CHECK (author IN ('agent', 'person')),
  profile_id TEXT,
  run_id TEXT REFERENCES runs (id),
  step_key TEXT,
  step_execution_id TEXT REFERENCES step_executions (id),
  source_events TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  edited_at INTEGER,
  CHECK ((author = 'agent') = (profile_id IS NOT NULL)),
  CHECK ((profile_id IS NULL) = (run_id IS NULL) AND (run_id IS NULL) = (step_key IS NULL)
    AND (step_key IS NULL) = (step_execution_id IS NULL))
) STRICT;
CREATE INDEX memories_by_task ON memories (task_id, status);
CREATE UNIQUE INDEX memories_active_keys ON memories (ifnull(task_id, ''), key)
  WHERE status = 'active' AND key IS NOT NULL;
CREATE INDEX memories_by_supersedes ON memories (supersedes_id);

-- Two external-content FTS5 indexes over text, key and tags, kept in step by triggers: words with
-- the default unicode61 tokenizer for BM25 ranking, and trigrams for other word forms, file paths
-- and identifiers inside longer ones (substring matches).
CREATE VIRTUAL TABLE memories_words USING fts5 (text, key, tags, content = 'memories', content_rowid = 'seq');
CREATE VIRTUAL TABLE memories_trigrams USING fts5 (text, key, tags, content = 'memories', content_rowid = 'seq',
  tokenize = 'trigram');
CREATE TRIGGER memories_indexed AFTER INSERT ON memories BEGIN
  INSERT INTO memories_words (rowid, text, key, tags) VALUES (new.seq, new.text, new.key, new.tags);
  INSERT INTO memories_trigrams (rowid, text, key, tags) VALUES (new.seq, new.text, new.key, new.tags);
END;
CREATE TRIGGER memories_unindexed AFTER DELETE ON memories BEGIN
  INSERT INTO memories_words (memories_words, rowid, text, key, tags) VALUES ('delete', old.seq, old.text, old.key,
    old.tags);
  INSERT INTO memories_trigrams (memories_trigrams, rowid, text, key, tags) VALUES ('delete', old.seq, old.text,
    old.key, old.tags);
END;
CREATE TRIGGER memories_reindexed AFTER UPDATE OF text, key, tags ON memories BEGIN
  INSERT INTO memories_words (memories_words, rowid, text, key, tags) VALUES ('delete', old.seq, old.text, old.key,
    old.tags);
  INSERT INTO memories_trigrams (memories_trigrams, rowid, text, key, tags) VALUES ('delete', old.seq, old.text,
    old.key, old.tags);
  INSERT INTO memories_words (rowid, text, key, tags) VALUES (new.seq, new.text, new.key, new.tags);
  INSERT INTO memories_trigrams (rowid, text, key, tags) VALUES (new.seq, new.text, new.key, new.tags);
END;
CREATE TRIGGER memories_keep_origin BEFORE UPDATE ON memories
WHEN NEW.seq IS NOT OLD.seq OR NEW.id IS NOT OLD.id OR NEW.task_id IS NOT OLD.task_id OR NEW.author IS NOT OLD.author
  OR NEW.profile_id IS NOT OLD.profile_id OR NEW.run_id IS NOT OLD.run_id OR NEW.step_key IS NOT OLD.step_key
  OR NEW.step_execution_id IS NOT OLD.step_execution_id OR NEW.source_events IS NOT OLD.source_events
  OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'a memory record keeps its scope and origin'); END;

-- Project settings › Memory, one row; a project that never changed it uses the defaults.
CREATE TABLE memory_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  task_writes TEXT NOT NULL CHECK (task_writes IN ('automatic', 'after_approval')),
  project_writes TEXT NOT NULL CHECK (project_writes IN ('automatic', 'after_approval')),
  max_records INTEGER NOT NULL CHECK (max_records BETWEEN 1 AND 50),
  max_tokens INTEGER NOT NULL CHECK (max_tokens BETWEEN 500 AND 20000)
) STRICT;

-- The records each invocation's prompt was given, in prompt order, with their estimated tokens,
-- written once before its agent starts. No reference to `memories`: a record a person deletes later
-- stays listed by its id, which then resolves to no record.
CREATE TABLE invocation_memories (
  invocation_id TEXT NOT NULL REFERENCES invocations (id),
  position INTEGER NOT NULL CHECK (position >= 0),
  memory_id TEXT NOT NULL,
  reason TEXT NOT NULL CHECK (reason IN ('handoff', 'pinned', 'decision', 'warning', 'match')),
  tokens INTEGER NOT NULL CHECK (tokens >= 0),
  PRIMARY KEY (invocation_id, position)
) STRICT;
";
