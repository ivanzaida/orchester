use orchester_core::integration::local::{Checkout, LocalError};

use super::{IntegrationError, LocalIntegration};

pub(super) const MESSAGE: &str = "Resolve task integration conflicts";

impl LocalIntegration {
  pub(super) fn reconcile_task_commit(&mut self, actual: &Checkout) -> Result<(), IntegrationError> {
    let before = self.operation.owned_task().ok_or(LocalError::Reconcile)?;
    let action = self.operation.task_commit_action().ok_or(LocalError::Reconcile)?;
    let mut expected = before.clone();
    expected.head.clone_from(&actual.head);
    expected.operation = None;
    expected.files.clear();
    if &expected != actual
      || self.task_git()?.text(&["rev-parse", "HEAD^1"])? != before.head
      || self.task_git()?.text(&["rev-parse", "HEAD^2"])? != self.operation.reviewed().head
      || !self
        .task_git()?
        .has_task_commit_receipt(&actual.head, action, MESSAGE)?
    {
      return Err(LocalError::Reconcile.into());
    }
    self.operation.own_task(actual.clone());
    self.operation.begin_check()?;
    self.save()?;
    self.run_check()
  }
}
