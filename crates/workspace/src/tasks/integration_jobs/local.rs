use super::{Active, BusyJob, RunJobs};
use crate::{messages::Message, workspace::OpenProject};

/// Keeps local checkout work visible to the existing window-close flow, and releases the queue on drop.
pub struct LocalWork {
  job: RunJobs,
}

impl RunJobs {
  pub fn local_work(&self, open: &OpenProject, what: Message) -> Option<LocalWork> {
    if !self.begin(Active::Local) {
      return None;
    }
    self.update(|state| {
      state.busy_with = Some(BusyJob {
        project: open.project.name().as_str().into(),
        what,
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
