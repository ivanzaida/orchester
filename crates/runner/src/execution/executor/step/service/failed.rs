//! Why one service did not start: the words its start's answer gives the agent, and, for a failure the person can act
//! on (Docker not available, a readiness timeout), what the run's conversation notes of it
//! (`orchester_core::service::ServiceStartFailure`, states 10a and 10c of `oXVuM`).

use orchester_core::service::StartFailureCause;

/// A service's failed start.
pub(in crate::execution::executor::step) struct Failed {
  /// What the start's answer says.
  pub(super) why: String,
  /// The cause the conversation notes, with the service's last lines once its program ran; `None` for a failure
  /// noted only in the answer.
  pub(super) shown: Option<(StartFailureCause, Vec<String>)>,
}

impl Failed {
  /// A failure the conversation notes as `cause`, with no lines yet.
  pub(super) const fn shown(why: String, cause: StartFailureCause) -> Self {
    Self {
      why,
      shown: Some((cause, Vec::new())),
    }
  }

  /// The same failure, its words turned by `words`.
  #[must_use]
  pub(super) fn worded(self, words: impl FnOnce(String) -> String) -> Self {
    Self {
      why: words(self.why),
      shown: self.shown,
    }
  }

  /// The same failure, `lines` the last lines its note shows.
  #[must_use]
  pub(super) fn with_lines(mut self, lines: Vec<String>) -> Self {
    if let Some((_, shown)) = &mut self.shown {
      *shown = lines;
    }
    self
  }
}

/// A failure noted only in the start's answer.
impl From<String> for Failed {
  fn from(why: String) -> Self {
    Self { why, shown: None }
  }
}
