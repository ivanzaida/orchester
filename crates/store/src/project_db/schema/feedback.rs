//! Agent feedback on pull requests (ORC-14, `orchester_core::pull_request::feedback`).

pub(super) const MIGRATION: &str = r"
-- A person's feedback threads on a pull request of the linked repository, for the agent of its linked task:
-- kept in Orchester, never posted to GitHub. A thread is anchored to lines of one side of a file, with the code
-- they held (`excerpt`, a JSON array of lines), or to the whole pull request (no path). Desktop-written.
CREATE TABLE feedback_threads (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks (id),
  host TEXT NOT NULL,
  owner TEXT NOT NULL,
  name TEXT NOT NULL,
  number INTEGER NOT NULL CHECK (number > 0),
  path TEXT CHECK (length(trim(path)) > 0),
  side TEXT CHECK (side IN ('old', 'new')),
  line_start INTEGER CHECK (line_start > 0),
  line_end INTEGER CHECK (line_end >= line_start),
  excerpt TEXT,
  recipient_profile TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  resolved_at INTEGER,
  CHECK ((path IS NULL) = (side IS NULL) AND (path IS NULL) = (line_start IS NULL)
    AND (path IS NULL) = (line_end IS NULL) AND (path IS NULL) = (excerpt IS NULL))
) STRICT;
CREATE INDEX feedback_threads_by_pull_request ON feedback_threads (host, owner, name, number, created_at);
CREATE TRIGGER feedback_threads_keep_anchor BEFORE UPDATE ON feedback_threads
WHEN NEW.id IS NOT OLD.id OR NEW.task_id IS NOT OLD.task_id OR NEW.host IS NOT OLD.host OR NEW.owner IS NOT OLD.owner
  OR NEW.name IS NOT OLD.name OR NEW.number IS NOT OLD.number OR NEW.path IS NOT OLD.path OR NEW.side IS NOT OLD.side
  OR NEW.line_start IS NOT OLD.line_start OR NEW.line_end IS NOT OLD.line_end OR NEW.excerpt IS NOT OLD.excerpt
  OR NEW.recipient_profile IS NOT OLD.recipient_profile OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'a feedback thread keeps what it is about'); END;
CREATE TRIGGER feedback_threads_no_delete BEFORE DELETE ON feedback_threads
BEGIN SELECT RAISE(ABORT, 'feedback threads are never deleted'); END;

-- The posts of a thread, a request and the replies after it, once per idempotency key. A post is held until it
-- is sent as a message of the task's conversation (`message_id`) or withdrawn; neither is undone, and what it
-- says never changes.
CREATE TABLE feedback_posts (
  id TEXT PRIMARY KEY,
  thread_id TEXT NOT NULL REFERENCES feedback_threads (id),
  idempotency_key TEXT NOT NULL UNIQUE,
  kind TEXT NOT NULL CHECK (kind IN ('question', 'fix')),
  body TEXT NOT NULL CHECK (length(trim(body)) > 0),
  revision TEXT NOT NULL CHECK (length(revision) = 40),
  written_at INTEGER NOT NULL,
  message_id TEXT UNIQUE REFERENCES messages (id),
  withdrawn_at INTEGER,
  CHECK (message_id IS NULL OR withdrawn_at IS NULL)
) STRICT;
CREATE INDEX feedback_posts_by_thread ON feedback_posts (thread_id, written_at);
CREATE TRIGGER feedback_posts_move_forward BEFORE UPDATE ON feedback_posts
WHEN NEW.id IS NOT OLD.id OR NEW.thread_id IS NOT OLD.thread_id OR NEW.idempotency_key IS NOT OLD.idempotency_key
  OR NEW.kind IS NOT OLD.kind OR NEW.body IS NOT OLD.body OR NEW.revision IS NOT OLD.revision
  OR NEW.written_at IS NOT OLD.written_at
  OR (OLD.message_id IS NOT NULL AND NEW.message_id IS NOT OLD.message_id)
  OR (OLD.withdrawn_at IS NOT NULL AND NEW.withdrawn_at IS NOT OLD.withdrawn_at)
BEGIN SELECT RAISE(ABORT, 'a feedback post keeps what it says, and is sent or withdrawn once'); END;
CREATE TRIGGER feedback_posts_no_delete BEFORE DELETE ON feedback_posts
BEGIN SELECT RAISE(ABORT, 'feedback posts are never deleted'); END;
";
