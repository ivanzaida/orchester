//! Where a sign-in stands: the password, or the code that finishes a sign-in the password started (`eWsUb`). The
//! challenge stays in memory, in the sign-in's step only, whose signal is made outside the component's context so
//! DevTools never records it; it is never logged, stored or shown.

use std::{
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
  time::Instant,
};

use lurq::app::component::DevtoolsInspectable;
use orchester_workspace::sharing::cloud::{Challenge, CodeKind};

/// Numbers each code entry, so what an earlier one came to is not shown for a later one.
static ENTRIES: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub(crate) enum Step {
  /// The email and the password; `timed_out` once a code step ended before a code finished it.
  Password {
    timed_out: bool,
  },
  Code(Entry),
}

impl PartialEq for Step {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Password { timed_out: one }, Self::Password { timed_out: two }) => one == two,
      (Self::Code(one), Self::Code(two)) => one.id == two.id,
      _ => false,
    }
  }
}

impl DevtoolsInspectable for Step {}

impl Step {
  /// The code step for `challenge`, with a code from the app.
  pub(super) fn code(challenge: Challenge) -> Self {
    Self::Code(Entry::new(Arc::new(challenge), CodeKind::App))
  }
}

/// One code entry of a sign-in: its challenge and the kind of code the person types.
#[derive(Clone)]
pub(crate) struct Entry {
  pub(super) challenge: Arc<Challenge>,
  pub(super) kind: CodeKind,
  pub(super) id: u64,
}

impl Entry {
  fn new(challenge: Arc<Challenge>, kind: CodeKind) -> Self {
    Self {
      challenge,
      kind,
      id: ENTRIES.fetch_add(1, Ordering::Relaxed),
    }
  }

  /// The same sign-in, with the other kind of code: a recovery code instead of the app's, and back.
  pub(super) fn switched(&self) -> Step {
    let kind = match self.kind {
      CodeKind::App => CodeKind::Recovery,
      CodeKind::Recovery => CodeKind::App,
    };
    Step::Code(Self::new(self.challenge.clone(), kind))
  }

  pub(super) fn recovery(&self) -> bool {
    self.kind == CodeKind::Recovery
  }

  /// Whether the cloud takes no code for it any more, by this computer's monotonic clock.
  pub(super) fn expired(&self) -> bool {
    self.challenge.ended_by(Instant::now())
  }
}
