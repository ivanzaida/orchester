//! A run's pull request CI on its task's page (ORC-335, `docs/design/pr-ci-status.md`, rows 10 `A56TC` and 11 `Wm7Nq`
//! of `C3wC8`): the pull request of the latest run's branch and its checks, read off the UI thread every 30 s while the
//! page is open ([`reader`]); on the Summary as the *Pull request* card above the run ([`card`]), in the header's
//! status line and primary action ([`header`]), and in the Conversation as event lines and a card among its rows by
//! time ([`timeline`]). *Fix CI* and *Fix CI automatically* go through [`actions`]. A run without a pull request shows
//! none of it.

pub(super) mod actions;
mod auto_row;
pub(super) mod card;
mod checks;
pub(super) mod header;
pub(super) mod reader;
pub(super) mod timeline;
mod words;

use std::sync::Arc;

use orchester_core::pull_request::ci::{
  CiSummary,
  fix::{RunFixes, failure_of},
};
use orchester_workspace::{
  pull_requests::{
    ci::{PullRequestCi, ReadCi, RunCi},
    ci_fix::{FixOffer, offer},
  },
  tasks::detail::TaskDetail,
};

/// What the page shows of `found`, read for `detail`'s latest run; `lost`: the run is recorded as running while no
/// runner looks after it. `None` while that run's branch has no pull request, or what was read is another run's.
pub fn shown(found: &reader::Found, detail: &TaskDetail, lost: bool) -> Option<ShownCi> {
  let (run, found) = found.0.as_ref()?;
  if detail.run.as_ref().map(orchester_workspace::tasks::detail::RunView::id) != Some(*run) {
    return None;
  }
  Some(ShownCi {
    offer: offer(detail, lost, &found.ci),
    found: found.clone(),
  })
}

/// The latest run's pull request with its CI as the page shows it, and what Fix CI offers for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShownCi {
  pub found: Arc<RunCi>,
  pub offer: FixOffer,
}

/// How the pull request's CI stands, as its badge and the header say it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
  /// A check is queued or running and none failed.
  Running,
  Passed,
  /// A check failed and no Fix CI answered this failure.
  Failed,
  /// A check failed and a Fix CI was sent for it: the agent works on it.
  Fixing,
  /// The head has no checks.
  NoChecks,
  /// The checks were not read: no `gh`, no sign-in, or GitHub did not give them.
  Unknown,
}

impl ShownCi {
  pub fn number(&self) -> u64 {
    self.found.pull_request.number()
  }

  /// The checks, when they were read.
  pub fn read(&self) -> Option<&ReadCi> {
    match &self.found.ci {
      PullRequestCi::Read(read) => Some(read),
      _ => None,
    }
  }

  /// How the checks stand, with the run's Fix CI `fixes`.
  pub fn look(&self, fixes: &RunFixes) -> Look {
    let Some(read) = self.read() else {
      return Look::Unknown;
    };
    match read.checks.summary() {
      CiSummary::None => Look::NoChecks,
      CiSummary::Pending => Look::Running,
      CiSummary::Passed => Look::Passed,
      CiSummary::Failed => {
        let answered = failure_of(&read.checks).is_some_and(|failure| fixes.sent_for(&failure).is_some());
        if answered { Look::Fixing } else { Look::Failed }
      }
    }
  }

  /// Whether the switch of *Fix CI automatically* can be used: CI is read.
  pub fn readable(&self) -> bool {
    self.read().is_some()
  }
}
