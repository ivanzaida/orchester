use orchester_core::project::TaskKey;

use super::{Active, BusyJob, RunJobs};
use crate::{messages::Message, workspace::OpenProject};

/// Keeps local checkout work visible to the existing window-close flow, and releases the queue on drop.
pub struct LocalWork {
  job: RunJobs,
}

/// A mounted managed review cannot be bypassed through authenticated legacy merge.
pub struct LocalDecision(RunJobs);
impl Drop for LocalDecision {
  fn drop(&mut self) {
    self.0.update(|state| state.local_decisions -= 1);
  }
}

impl RunJobs {
  pub fn local_decision(&self) -> Option<LocalDecision> {
    let mut started = false;
    self.jobs.0.send_if_modified(|project| {
      if project.runs.values().any(|run| run.active == Active::Merge) {
        return false;
      }
      project.runs.entry(self.run).or_default().local_decisions += 1;
      started = true;
      true
    });
    if started {
      self.jobs.changed();
      Some(LocalDecision(self.clone()))
    } else {
      None
    }
  }

  pub fn local_work(&self, open: &OpenProject, task: &TaskKey) -> Option<LocalWork> {
    if !self.begin(Active::Local) {
      return None;
    }
    self.update(|state| {
      state.busy_with = Some(BusyJob {
        project: open.project.name().as_str().into(),
        what: Message::new("localIntegration.running").with("task", task.to_string()),
        merge: true,
      })
    });
    Some(LocalWork { job: self.clone() })
  }
}

impl Drop for LocalWork {
  fn drop(&mut self) {
    self.job.finish();
  }
}
