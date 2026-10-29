//! Off-thread saves using workspace services, and the page's read/save feedback.
use std::{
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction, FutureState},
  },
  core::Signal,
  node::Element,
};
use orchester_core::{
  id::{ConnectionId, ProjectId},
  limits::ProjectLimits,
  run::{Answerer, ProjectRuleId, review::AutoModeSettings},
};
use orchester_workspace::{
  background, code_index,
  connections::github::expect_active_account_in,
  messages::{self, Message},
  project_settings::{self, Settings},
  tasks::outside::{Outside, OutsideCounts},
};
use parking_lot::Mutex;

use super::{Status, TabProps};
use crate::{
  project::{ProjectFeed, own::OwnChanges},
  services::Services,
  ui::{field::field_error, message::message_text, skeleton::Skeleton, state::load_error},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read(pub Settings, pub u64);
impl DevtoolsInspectable for Read {}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
  Limits {
    base: ProjectLimits,
    asked: ProjectLimits,
  },
  Files(bool),
  Index(bool),
  /// *Tell working agents when the base branch moves*.
  BaseNotices(bool),
  /// Manual's reviewer.
  Auto {
    base: AutoModeSettings,
    asked: AutoModeSettings,
    /// The input that made the change may choose another reviewer: a person's, or automation's under both opt-ins.
    confirmed: bool,
  },
  /// *Allow Autopilot for agents in this project*, turned on by a person, or off by anyone.
  Autopilot {
    on: bool,
    by: Answerer,
  },
  UseAccount(ConnectionId),
  /// Revokes a rule of the project's (*Approve for every run of this project*).
  Revoke(ProjectRuleId),
}
/// The settings as the save read them after writing, and the project's change counts right after the save
/// recorded its change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved(pub Settings, pub OutsideCounts);
impl DevtoolsInspectable for Saved {}
pub type Save = FutureAction<(PathBuf, Change), Saved, Message>;

/// Saves the settings of `project` as asked.
pub fn action(ctx: &mut Ctx, project: ProjectId) -> Save {
  let services = Services::of(ctx);
  ctx.future_action(move |(root, change): (PathBuf, Change)| {
    // The save reads the settings again after writing them, for the tab.
    tracing::trace!(target: crate::app_log::READ_LOG, read = "project settings");
    let (runners, changes) = (services.runners.clone(), services.changes.clone());
    async move {
      background::run(move || {
        let outside = match &change {
          Change::Limits { .. } => Outside::Limits,
          Change::Auto { .. } | Change::Autopilot { .. } | Change::Revoke(_) => Outside::AutoMode,
          Change::Index(_) => Outside::CodeIndex,
          Change::Files(_) | Change::BaseNotices(_) => Outside::ProjectSettings,
          Change::UseAccount(_) => Outside::Connections,
        };
        match change {
          Change::Limits { base, asked } => {
            project_settings::save_limits_in(&root, &runners, base, asked).map_err(|e| messages::store_error(&e))?;
          }
          Change::Files(on) => {
            project_settings::save_agent_files_in(&root, on).map_err(|e| messages::store_error(&e))?
          }
          Change::BaseNotices(on) => {
            project_settings::save_base_notices_in(&root, on).map_err(|e| messages::store_error(&e))?
          }
          Change::Index(on) => code_index::save_setting_in(
            &root,
            if on {
              orchester_core::code_index::CodeIndexSetting::On
            } else {
              orchester_core::code_index::CodeIndexSetting::Off
            },
          )
          .map_err(|e| messages::store_error(&e))?,
          Change::Auto { base, asked, confirmed } => {
            project_settings::save_auto_mode_in(&root, &base, &asked, confirmed)?
          }
          Change::Autopilot { on, by } => project_settings::save_allow_autopilot_in(&root, on, by).map(|_| ())?,
          Change::UseAccount(id) => expect_active_account_in(&root, id).map_err(|e| e.message())?,
          Change::Revoke(rule) => {
            project_settings::revoke_project_rule_in(&root, rule).map_err(|e| messages::store_error(&e))?
          }
        }
        // Right after the write, whether or not the tab is still there to take the save: every screen that
        // shows the change reads it again.
        let counts = changes.record(project, outside);
        project_settings::load_in(&root)
          .map(|settings| Saved(settings, counts))
          .map_err(|e| messages::store_error(&e))
      })
      .await?
    }
  })
}
/// What a tab's own saves leave for its reads: the settings a save read right after writing, which the read the
/// save asks for takes instead of reading them again, and the changes of the kinds the tab shows, its own too.
#[derive(Clone)]
pub struct OwnSaves {
  read: Arc<Mutex<Option<Settings>>>,
  changes: OwnChanges,
  /// How many of the tab's saves run now.
  saving: Signal<u32>,
  /// The count the tab last read at, which holds while a save of its own runs.
  last: Arc<Mutex<u64>>,
}

impl OwnSaves {
  pub fn new(ctx: &mut Ctx, kinds: &'static [Outside]) -> Self {
    Self {
      read: Arc::default(),
      changes: OwnChanges::new(ctx, kinds),
      saving: ctx.signal(0),
      last: Arc::default(),
    }
  }

  /// Follows a save of the tab's as it runs (`pending`) and ends; `begun` is that save's own mark.
  pub fn saving(&self, pending: bool, begun: &AtomicBool) {
    if pending != begun.swap(pending, Ordering::Relaxed) {
      self.saving.update(|saving| {
        *saving = if pending { *saving + 1 } else { saving.saturating_sub(1) };
      });
    }
  }

  /// How often the kinds the tab shows changed. Reading it in a render renders again when it moves. While a save of
  /// the tab's runs, it holds: the save records its change before the tab takes it, and a change counted
  /// meanwhile is read once the save ended. Once a change was counted after the tab's own save, what that save
  /// read is not taken: the tab reads the settings again.
  pub fn count(&self, feed: &ProjectFeed) -> u64 {
    let count = self.changes.count(feed);
    if self.saving.get() > 0 {
      return *self.last.lock();
    }
    if self.changes.newer(feed) {
      self.read.lock().take();
    }
    *self.last.lock() = count;
    count
  }

  /// Takes a save of the tab's own: the tab's next read is what the save read, and the change the save recorded
  /// is seen, so neither the tab's revision nor the feed catching up reads the settings again. A change made
  /// after the save recorded its own still reads them again.
  pub fn took(&self, saved: &Saved) {
    self.changes.saw(saved.1);
    *self.read.lock() = Some(saved.0.clone());
  }
}

pub fn load(ctx: &mut Ctx, root: PathBuf, revision: u64, own: &OwnSaves) -> FutureState<Read, Message> {
  let saved = Arc::clone(&own.read);
  let hold = crate::services::Services::of(ctx).hold;
  let mut state = ctx
    .future((root, revision), move |(root, revision)| {
      let saved = saved.lock().take();
      if saved.is_none() {
        tracing::trace!(target: crate::app_log::READ_LOG, read = "project settings");
      }
      let hold = hold.clone();
      async move {
        if let Some(settings) = saved {
          return Ok(Read(settings, revision));
        }
        hold.released().await;
        background::run(move || project_settings::load_in(&root).map(|settings| Read(settings, revision)))
          .await?
          .map_err(|e| messages::store_error(&e))
      }
    })
    .state()
    .get();
  // Futures retain their previous data when dependencies change. It is no longer a fresh read.
  if state.data.as_ref().is_some_and(|read| read.1 != revision) {
    state.data = None;
  }
  state
}
pub fn follow(
  ctx: &mut Ctx,
  save: &Save,
  status: Signal<Status>,
  (revision, stored, own): (Signal<u64>, Signal<Option<Read>>, OwnSaves),
) {
  let project = ctx.props::<TabProps>().project.clone();
  let begun = AtomicBool::new(false);
  ctx.watch(&save.state(), move |state| {
    own.saving(state.is_pending(), &begun);
    if state.is_pending() {
      status.set(Status::Saving);
      return;
    }
    if let Some(error) = &state.error {
      tracing::warn!(key=%error.key, "project settings save failed");
      status.set(Status::Failed);
    } else if let Some(saved) = &state.data {
      stored.set(Some(Read(saved.0.clone(), u64::MAX)));
      status.set(Status::Saved);
      own.took(saved);
      revision.update(|r| *r += 1);
      project.feed.saved();
    }
  });
}
pub fn start(save: &Save, root: PathBuf, change: Change) -> Arc<dyn Fn() + Send + Sync> {
  let save = save.clone();
  Arc::new(move || {
    if !save.state().get_untracked().is_pending() {
      save.run((root.clone(), change.clone()));
    }
  })
}
pub fn error(ctx: &Ctx, save: &Save) -> Option<Element> {
  save
    .state()
    .get()
    .error
    .map(|e| field_error("settings-save-error", &message_text(ctx, &e)).into())
}
/// What a tab shows before its settings are read: why reading failed with Try again, or `bones` in its skeleton,
/// named *Reading project settings…* (`project_settings.reading`).
pub fn pending(
  ctx: &mut Ctx,
  state: &FutureState<Read, Message>,
  revision: &Signal<u64>,
  bones: impl Into<Element>,
) -> Element {
  match &state.error {
    Some(error) => {
      let revision = revision.clone();
      load_error(
        ctx,
        "settings-load-error",
        (&ctx.t("project_settings.loadFailed"), &message_text(ctx, error)),
        move || revision.update(|r| *r += 1),
      )
    }
    None => Skeleton::new("settings-loading", ctx.t("project_settings.reading")).show(ctx, bones),
  }
}
