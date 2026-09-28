//! Keeping a child's process tree together so it can be ended as a whole.
//!
//! - Windows ([`job`]): a kill-on-close Job Object per child. Closing it ends the child and every process it started,
//!   and nothing survives the runner.
//! - macOS ([`group`]): the child leads a process group of its own (`orchester_exec::process_group`). Ending it sends
//!   the group SIGTERM, then SIGKILL after a grace period; whatever the child left running when it exited is ended the
//!   same way. A process that leaves the group on purpose (`setsid`) is not ended.

#[cfg(unix)]
mod group;
#[cfg(windows)]
mod job;

#[cfg(unix)]
pub(super) use self::group::Containment;
#[cfg(windows)]
pub(super) use self::job::Containment;
