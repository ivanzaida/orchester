//! The process tree of a running `tools/qa-app.ps1`: everything the script starts, the QA app among them, so a start
//! that is given up (its turn ended, its run stopped, or the script named no app) ends all of it, and one that
//! succeeded lets the app run on.
//!
//! - Windows ([`windows`]): a kill-on-close Job Object. The runner closing it, or the runner ending, ends the tree;
//!   [`Tree::keep`] clears kill-on-close first.
//! - macOS ([`unix`]): the script leads a process group of its own (`orchester_exec::prepare`); ending the tree ends
//!   the group. A runner that is killed while the script runs leaves the group running (see `AGENTS.md`'s known gaps).

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub(super) use self::unix::Tree;
#[cfg(windows)]
pub(super) use self::windows::Tree;
