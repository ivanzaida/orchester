//! Warm workspaces (ORC-114): which workspace a borrower takes, warm folder patterns and suggestions, the disk guard
//! and the cleanup order.

mod cleanup;
mod disk;
mod pool;
mod suggest;
mod warm;

use std::num::NonZeroU32;

use orchester_core::{
  time::Timestamp,
  workspaces::{WorkspaceNumber, WorkspaceRecord},
};

fn number(n: u32) -> WorkspaceNumber {
  WorkspaceNumber::new(n).expect("a workspace number above zero")
}

fn limit(n: u32) -> NonZeroU32 {
  NonZeroU32::new(n).expect("a limit above zero")
}

/// An idle workspace `n`, last used at `minute`.
fn idle(n: u32, minute: i64) -> WorkspaceRecord {
  WorkspaceRecord {
    last_used: Some(Timestamp::from_unix_millis(minute * 60_000)),
    ..WorkspaceRecord::new(number(n))
  }
}
