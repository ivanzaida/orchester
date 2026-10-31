//! This copy's updates in the window (`Auto-updater · redesign` `acFkz`): Settings › General's *Updates* group
//! (`wSMNG`), drawn from plain [`UpdatesFacts`] by [`UpdatesSection`] so a test can hand it any state, its presses
//! going back as an [`UpdateAction`]. Settings gathers the facts and does what the actions ask; the status bar's item
//! is `shell::status_bar::update`, the question about the runs at work `shell::update`, and the notification of a ready
//! update `notifications`.

mod parts;
mod row;
mod section;

use std::{path::PathBuf, sync::Arc};

use orchester_core::{
  app_update::{Channel, UpdateSettings},
  time::Timestamp,
};
use orchester_workspace::app_updates::{UpdateState, install::JustUpdated};

pub use self::{
  row::{ROW, channel_name},
  section::{GROUP, SectionProps, UpdatesSection},
};
use crate::routes::UpdatesShow;

/// What *Updates* shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdatesFacts {
  /// This copy's version.
  pub running: String,
  pub state: UpdateState,
  /// The update this start finished, whose notes *What's new* shows once nothing newer is ready.
  pub just_updated: Option<JustUpdated>,
  /// The channel and whether updates install by themselves; `None` while they are read.
  pub settings: Option<UpdateSettings>,
  /// When Orchester Cloud last answered a check: `None` while it is read, `Some(None)` before it ever did.
  pub last_checked: Option<Option<Timestamp>>,
  /// Why the settings can't be read or a change was not saved, or a page did not open, in the person's words.
  pub problem: Option<String>,
  /// What the route asked to show: the section in view, with the release notes open.
  pub show: Option<UpdatesShow>,
}

/// What a press in *Updates* asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateAction {
  /// *Check now*, and *Try again* after a failed download.
  CheckNow,
  Pause,
  Resume,
  /// *Restart to update* and *Install now*: install at once with no run at work, else ask (`MGILm`).
  Restart,
  Channel(Channel),
  Automatic(bool),
  /// A web page: the newest release's download, or a link in the release notes.
  Open(String),
  /// The installer's log of an update that did not install.
  OpenLog(PathBuf),
}

/// Where the section's actions go.
pub type UpdateDispatch = Arc<dyn Fn(UpdateAction) + Send + Sync>;
