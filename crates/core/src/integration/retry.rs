//! Which merges Orchester tries again by itself, and when (`crates/desktop-app/design/orchester.pen`, board
//! `Review · redesign`, 09j): one that did not land for a cause that passes by itself — the project's integration lease
//! another operation holds, a file another program holds open, the project database busy — is tried again up to
//! [`RETRIES`] times, [`delay_before`] each try. A lasting cause, such as a disk's I/O error, Git refusing or a
//! checkout that is not the one recorded, waits for a person.

use std::{num::NonZeroU32, time::Duration};

use super::{GitError, MergeError, PreflightCause};
use crate::command::CommandError;

/// The most tries Orchester makes of a merge after the one that was asked for.
pub const RETRIES: u32 = 3;

/// The OS's error numbers of a file another program holds: a sharing or lock violation on Windows, `EBUSY` and
/// `EAGAIN` on Unix.
#[cfg(windows)]
const HELD: [i32; 2] = [32, 33];
#[cfg(target_os = "macos")]
const HELD: [i32; 2] = [16, 35];
#[cfg(all(unix, not(target_os = "macos")))]
const HELD: [i32; 2] = [16, 11];

/// What Git and the OS say of a file another program holds, or of the index another git holds locked, in English
/// ([`crate::diff::git::git_command`]).
const HELD_WORDS: [&str; 5] = [
  "Device or resource busy",
  "Resource busy",
  "Resource temporarily unavailable",
  "being used by another process",
  "Another git process seems to be running",
];

/// How long Orchester waits before its try number `retry`: 10 s before the first, 30 s before each after it.
pub fn delay_before(retry: NonZeroU32) -> Duration {
  if retry.get() == 1 {
    Duration::from_secs(10)
  } else {
    Duration::from_secs(30)
  }
}

/// Whether the OS's error `code` says another program holds the file.
pub fn held_by_another_program(code: i32) -> bool {
  HELD.contains(&code)
}

impl PreflightCause {
  /// Whether the cause passes by itself: the lease another operation holds, or a file another program holds open.
  pub fn passes(&self) -> bool {
    match self {
      Self::Held => true,
      Self::Io { code: Some(code), .. } => held_by_another_program(*code),
      Self::Git(cause) => held_words(cause.tail()),
      Self::Io { code: None, .. } | Self::State(_) | Self::Reason(_) => false,
    }
  }
}

impl MergeError {
  /// Whether the merge stopped for a cause that passes by itself: git, or the program that runs it, told of a file
  /// another program holds. A merge that left files changed waits for them to be put back.
  pub fn passes(&self) -> bool {
    match self {
      Self::Refused(said) | Self::Git(GitError::Failed(said) | GitError::Run(CommandError::Failed(said))) => {
        held_words(said)
      }
      _ => false,
    }
  }
}

fn held_words(said: &str) -> bool {
  HELD_WORDS.iter().any(|words| said.contains(words))
}
