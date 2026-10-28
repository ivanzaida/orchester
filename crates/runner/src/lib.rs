//! The runner process of one project, as described in `docs/architecture/storage.md`: it holds
//! the project's runner lock, serves the desktop over IPC, executes runs (worktrees, agent CLIs,
//! checkpoints) and records them in the project database.

pub mod approval_relay;
pub mod execution;
mod git;
pub mod lock;
mod lock_pause;
pub mod log_file;
pub mod notify;
pub mod private_file;
pub mod server;
pub mod shutdown_signal;
pub mod supervisor;
pub mod worktree;

/// The async workers of the runner's runtime. Agents' output, the desktop's requests, the relays and the timers run
/// on them; database and git calls block one only briefly, since a statement that waits for a database lock moves
/// off it first (`lock_pause`), and longer work (a codebase index build) runs on the blocking pool.
pub const WORKER_THREADS: usize = 2;
