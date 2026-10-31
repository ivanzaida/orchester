//! The project's browser as the scheduler holds it ([`crate::browser`]): started by its runs' first use, each run's
//! tabs closed as the run ends (its task settles it completed, cancelled or failed, or a person cancels or stops it
//! while no task runs it), and stopped by the server as the runner exits.

use std::{path::PathBuf, sync::Arc};

use super::Scheduler;
use crate::browser::ProjectBrowser;

impl Scheduler {
  /// The project's browser, which its runs' tabs are opened in.
  pub fn browser(&self) -> &Arc<ProjectBrowser> {
    &self.inner.browser
  }

  /// This scheduler's browser on the profile folder `profile`, before it is shared: what tests give it instead of
  /// the project's folder in the application data.
  #[must_use]
  pub fn with_browser_profile(self, profile: PathBuf) -> Self {
    self.with_browser(ProjectBrowser::new(profile))
  }

  /// This scheduler with `browser`, before it is shared: what tests give it to drive the browser's clock.
  #[must_use]
  pub fn with_browser(mut self, browser: ProjectBrowser) -> Self {
    if let Some(inner) = Arc::get_mut(&mut self.inner) {
      inner.browser = Arc::new(browser);
    }
    self
  }
}
