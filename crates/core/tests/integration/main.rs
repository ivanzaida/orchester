//! Integration of a finished run into the user's checkout, against real temporary git
//! repositories: merging it into the branch it started from, beside uncommitted work too, and
//! clearing its worktrees and branches.

mod cleanup;
mod forgotten;
mod fork;
mod merge;
mod merge_beside;
mod merge_guards;
mod merged;
mod recorded;
mod resolve;
mod support;
