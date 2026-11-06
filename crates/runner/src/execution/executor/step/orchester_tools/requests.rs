//! What the request of a call that runs a project program names, and what its start may run once approved: a QA
//! app's script, by its git blob, and a service start's services, each by its fingerprint
//! (`orchester_core::service::ServiceApproval`).

use orchester_core::{
  orchester_tools::{QaAppStart, QaApproval, ToolCall},
  service::{ServiceApproval, start_target},
};

use super::{
  super::{qa_app::script_blob, service::Started},
  ToolHost,
};
use crate::git::Git;

impl ToolHost {
  /// What the approval of `start` names: the QA script's git blob and the build or start scripts changed since the
  /// run's base revision, committed or not. What git cannot tell is left out, and the request says less.
  pub(super) fn qa_approval(&self, start: &QaAppStart) -> QaApproval {
    let git = Git::new(&self.worktree);
    let script = script_blob(&self.worktree).unwrap_or_default();
    let mut changed: Vec<String> = [
      git.stdout(&["diff", "--name-only", &self.base, "--"]),
      git.stdout(&["ls-files", "--others", "--exclude-standard"]),
    ]
    .into_iter()
    .filter_map(|listed| {
      listed
        .inspect_err(|error| tracing::warn!(%error, "the changed build scripts cannot be listed"))
        .ok()
    })
    .flat_map(|listed| listed.lines().map(str::to_owned).collect::<Vec<_>>())
    .filter(|path| QaApproval::watches(path))
    .collect();
    changed.sort();
    changed.dedup();
    QaApproval {
      script: script.trim().to_owned(),
      scenario: start.scenario().to_owned(),
      changed,
    }
  }

  /// What the request of `call`, a start of `started`, names: every service it runs with its program, arguments,
  /// variable names and fingerprint, kept so the start runs only those definitions. Why it is refused, before anybody
  /// is asked, when the project's services do not let it start.
  pub(super) fn service_target(&self, call: &ToolCall, started: Started<'_>) -> Result<String, String> {
    let services = self
      .services
      .as_ref()
      .ok_or_else(|| "this step has no services".to_owned())?;
    let approvals = services.service_approvals(started)?;
    let target = start_target(&approvals);
    self.service_starts.lock().push((call.clone(), approvals));
    Ok(target)
  }

  /// The services the latest look at `call` named, which its approval approved; earlier looks were denied or never
  /// run. None when it was never looked at, so the start refuses every service.
  pub(super) fn approved_services(&self, call: &ToolCall) -> Vec<ServiceApproval> {
    let mut looked = self.service_starts.lock();
    let approved = looked
      .iter()
      .rposition(|(at, _)| at == call)
      .map(|at| looked[at].1.clone())
      .unwrap_or_default();
    looked.retain(|(at, _)| at != call);
    approved
  }
}
