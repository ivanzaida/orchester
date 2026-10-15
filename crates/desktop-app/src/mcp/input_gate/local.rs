//! A queued local action never acquires native origin when the callback's thread-local scope ends.
use lurq::app::events::InputSource;
use orchester_workspace::tasks::local_integration::LocalOrigin;

use super::{InputGate, Ordering};

#[derive(Clone)]
pub struct LocalInput {
  gate: InputGate,
  origin: LocalOrigin,
  epoch: usize,
  consent_revision: Option<(u64, u64)>,
  admitted: bool,
}

impl InputGate {
  pub fn local_input(&self, source: InputSource) -> LocalInput {
    let origin = if source == InputSource::Synthetic {
      LocalOrigin::WindowAutomation
    } else {
      LocalOrigin::Native
    };
    // Its own statement, so the workspace lock is let go before `protecting` takes it again: a lock guard made inside
    // the literal below would live until the literal ends, and parking_lot's locks are not reentrant.
    let consent_revision = self.consent_revision();
    LocalInput {
      gate: self.clone(),
      origin,
      epoch: self.0.local_epoch.load(Ordering::Acquire),
      consent_revision,
      admitted: origin == LocalOrigin::Native || (self.0.window_control.load(Ordering::Acquire) && !self.protecting()),
    }
  }

  fn consent_revision(&self) -> Option<(u64, u64)> {
    self
      .0
      .workspace
      .lock()
      .as_ref()
      .map(|workspace| workspace.automation_consent_revision())
  }

  /// The existing server scope is independent of the two widening/approval opt-ins.
  pub fn bind_window_control(&self, allowed: bool) {
    if self.0.window_control.swap(allowed, Ordering::AcqRel) != allowed {
      self.0.local_epoch.fetch_add(1, Ordering::AcqRel);
    }
  }
}

impl LocalInput {
  pub const fn origin(&self) -> LocalOrigin {
    self.origin
  }

  pub fn allowed(&self) -> bool {
    self.admitted
      && (self.origin == LocalOrigin::Native
        || (self.gate.0.window_control.load(Ordering::Acquire)
          && !self.gate.protecting()
          && self.epoch == self.gate.0.local_epoch.load(Ordering::Acquire)
          && self.consent_revision == self.gate.consent_revision()))
  }

  /// Read applicable consent again immediately before queued work; failed reads cannot authorize input.
  pub fn recheck(&self) -> bool {
    if self.origin == LocalOrigin::Native {
      return self.admitted;
    }
    let workspace = self.gate.0.workspace.lock().clone();
    if let Some(workspace) = workspace
      && (workspace.automation_changes_permissions().is_err() || workspace.automation_answers_approvals().is_err())
    {
      workspace.invalidate_automation_consent();
      self.gate.refresh_tools();
      return false;
    }
    self.gate.refresh_tools();
    self.allowed()
  }
}
