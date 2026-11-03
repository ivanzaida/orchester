//! A person's review of a run's diff (ORC-366, `orchester_core::review`): threads, their messages and the files marked
//! *Viewed* at a head.

pub(super) const MIGRATION: &str = r"
-- A thread of a person's review of a run, anchored to lines of one side of a file of the run's diff at the head they
-- read, with the code those lines held (`excerpt`, a JSON array of lines). Kept once resolved. Desktop-written.
CREATE TABLE review_threads (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs (id),
  kind TEXT NOT NULL CHECK (kind IN ('comment', 'question')),
  path TEXT NOT NULL CHECK (length(trim(path)) > 0),
  side TEXT NOT NULL CHECK (side IN ('old', 'new')),
  line_start INTEGER NOT NULL CHECK (line_start > 0),
  line_end INTEGER NOT NULL CHECK (line_end >= line_start),
  excerpt TEXT NOT NULL,
  head TEXT NOT NULL CHECK (length(head) = 40),
  created_at INTEGER NOT NULL,
  resolved_at INTEGER
) STRICT;
CREATE INDEX review_threads_by_run ON review_threads (run_id, created_at);
CREATE TRIGGER review_threads_keep_anchor BEFORE UPDATE ON review_threads
WHEN NEW.id IS NOT OLD.id OR NEW.run_id IS NOT OLD.run_id OR NEW.kind IS NOT OLD.kind OR NEW.path IS NOT OLD.path
  OR NEW.side IS NOT OLD.side OR NEW.line_start IS NOT OLD.line_start OR NEW.line_end IS NOT OLD.line_end
  OR NEW.excerpt IS NOT OLD.excerpt OR NEW.head IS NOT OLD.head OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'a review thread keeps what it is about'); END;

-- The messages of a thread in order (`position`): the person's, with how asking or sending went, and the implementing
-- agent's words (`said`, a JSON document). A comment not sent yet is changed or deleted by the person; the agent's
-- words grow while its turn writes them (the runner's). Desktop- and runner-written.
CREATE TABLE review_messages (
  id TEXT PRIMARY KEY,
  thread_id TEXT NOT NULL REFERENCES review_threads (id),
  position INTEGER NOT NULL CHECK (position >= 0),
  said TEXT NOT NULL,
  body TEXT NOT NULL,
  at INTEGER NOT NULL,
  UNIQUE (thread_id, position)
) STRICT;

-- The files of a run's diff the person marked Viewed, per head.
CREATE TABLE review_viewed (
  run_id TEXT NOT NULL REFERENCES runs (id),
  head TEXT NOT NULL CHECK (length(head) = 40),
  path TEXT NOT NULL CHECK (length(trim(path)) > 0),
  at INTEGER NOT NULL,
  PRIMARY KEY (run_id, head, path)
) STRICT;
";
