//! Warm workspaces (ORC-114): the project's reusable folders `.orchester/workspaces/<number>`, who holds each and
//! what it holds, the runs waiting for one, the warm folders a switch keeps and the disk guard's free space. Runs that
//! started before keep their own worktree in `.orchester/worktrees/` until they end, so nothing older moves.

pub(super) const MIGRATION: &str = r"
-- One row per workspace made so far. `holder` is the JSON of who works there (NULL while idle); `last_used_at` when
-- it was last taken or given back; `last_run_id` the run that worked there last, which takes it first; `branch` the
-- branch checked out there last (NULL for the base branch). `stranded_*`: a run that stopped unexpectedly left this
-- many uncommitted files there, which a person keeps in a branch or discards before anyone takes it.
CREATE TABLE workspaces (
  number INTEGER PRIMARY KEY CHECK (number > 0),
  holder TEXT,
  last_used_at INTEGER,
  last_run_id TEXT,
  branch TEXT,
  warmth TEXT NOT NULL DEFAULT 'cold' CHECK (warmth IN ('cold', 'warm')),
  stranded_run_id TEXT,
  stranded_files INTEGER CHECK (stranded_files >= 0),
  CHECK (stranded_run_id IS NULL OR stranded_files IS NOT NULL)
) STRICT;

-- Runs that borrow workspaces: every run made since workspaces. One without a row has its own worktree in
-- .orchester/worktrees/ until it ends, and never goes on without it.
CREATE TABLE borrowing_runs (
  run_id TEXT PRIMARY KEY REFERENCES runs (id)
) STRICT;

-- Runs whose next step waits for a free workspace, since when.
CREATE TABLE workspace_waits (
  run_id TEXT PRIMARY KEY REFERENCES runs (id),
  since INTEGER NOT NULL
) STRICT;

-- Project settings › Runs: the free space the disk guard keeps and the optional warm-up command. Without a row the
-- project keeps 150 GB free and has no warm-up.
CREATE TABLE workspace_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  keep_free_gb INTEGER NOT NULL CHECK (keep_free_gb > 0),
  warm_up_command TEXT
) STRICT;

-- The warm folders, in the order the person sees them: kept between runs or cleaned every run, and when the
-- person's checkout last had one.
CREATE TABLE warm_folders (
  pattern TEXT PRIMARY KEY,
  position INTEGER NOT NULL,
  choice TEXT NOT NULL CHECK (choice IN ('keep', 'clean')),
  last_seen_at INTEGER
) STRICT;

-- The last warm-up that failed, until one succeeds: where, how and when, and its log file in the run storage.
CREATE TABLE warm_up_failures (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  workspace INTEGER NOT NULL CHECK (workspace > 0),
  exit_code INTEGER,
  failed_at INTEGER NOT NULL,
  log TEXT NOT NULL
) STRICT;
";
