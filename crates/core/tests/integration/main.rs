//! Integration of a finished run into the user's checkout, against real temporary git
//! repositories: merging it into the branch it started from, and clearing its worktrees and
//! branches.

mod cleanup;
mod merge;
mod recorded;
mod support;
