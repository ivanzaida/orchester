//! When a pull request was merged or closed and whether its head is a fork's (`NULL` and `0` for the ones a
//! sync stored before, until the next sync), and how the latest sync of a repository went
//! (`pull_request_attempts`): when it ran and, when it read nothing, why, as the Pull requests page says it.
//! Kept apart from `pull_request_syncs`, which says when the list shown was read.

pub(super) const MIGRATION: &str = r"
ALTER TABLE pull_requests ADD COLUMN merged_at INTEGER;
ALTER TABLE pull_requests ADD COLUMN closed_at INTEGER;
ALTER TABLE pull_requests ADD COLUMN cross_repository INTEGER NOT NULL DEFAULT 0 CHECK (cross_repository IN (0, 1));

CREATE TABLE pull_request_attempts (
  host TEXT NOT NULL,
  owner TEXT NOT NULL,
  name TEXT NOT NULL,
  attempted_at INTEGER NOT NULL,
  failure TEXT CHECK (failure IN (
    'rate_limited', 'account', 'unreachable', 'server_error', 'not_found', 'no_access', 'not_signed_in',
    'unrecognized'
  )),
  account_state TEXT CHECK (account_state IN (
    'not_checked', 'signed_in', 'not_signed_in', 'sign_in_unknown', 'wrong_account', 'missing_scope',
    'program_missing', 'unrecognized', 'could_not_run'
  )),
  login TEXT,
  expected_login TEXT,
  resets_at INTEGER,
  http_status INTEGER CHECK (http_status BETWEEN 100 AND 599),
  PRIMARY KEY (host, owner, name),
  CHECK ((failure = 'account') = (account_state IS NOT NULL))
) STRICT;
";
