//! The number of the user this process runs as, which names a user's own folders under `/tmp` on Unix (Claude Code's
//! `/tmp/claude-<uid>`). Windows names no folder by such a number: there it is `None`.

#[cfg(unix)]
pub fn user_id() -> Option<u32> {
  Some(rustix::process::getuid().as_raw())
}

#[cfg(windows)]
pub const fn user_id() -> Option<u32> {
  None
}
