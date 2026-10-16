//! Off-thread saves using workspace services, and the page's read/save feedback.
use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction, FutureState},
  },
  core::Signal,
  node::Element,
};
use orchester_core::{id::ConnectionId, limits::ProjectLimits, run::review::AutoModeSettings};
use orchester_workspace::{
  background, code_index,
  connections::github::expect_active_account_in,
  messages::{self, Message},
  project_settings::{self, Settings},
  tasks::outside::Outside,
};
use parking_lot::Mutex;

use super::{Status, TabProps};
use crate::{
  project::{ProjectFeed, own::OwnChanges},
  services::Services,
  ui::{
    field::field_error,
    message::message_text,
    state::{load_error, loading},
  },
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
  Auto {
    base: AutoModeSettings,
    asked: AutoModeSettings,
    confirmed: bool,
  },
  UseAccount(ConnectionId),
}
/// The settings as the save read them after writing, and the kind of change it made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved(pub Settings, pub Outside);
impl DevtoolsInspectable for Saved {}
pub type Save = FutureAction<(PathBuf, Change), Saved, Message>;

pub fn action(ctx: &mut Ctx) -> Save {
  let services = Services::of(ctx);
  ctx.future_action(move |(root, change): (PathBuf, Change)| {
    // The save reads the settings again after writing them, for the tab.
    tracing::trace!(target: crate::app_log::READ_LOG, read = "project settings");
    let runners = services.runners.clone();
    async move {
      background::run(move || {
        let outside = match &change {
          Change::Limits { .. } => Outside::Limits,
          Change::Auto { .. } => Outside::AutoMode,
          Change::Index(_) => Outside::CodeIndex,
          Change::Files(_) => Outside::ProjectSettings,
          Change::UseAccount(_) => Outside::Connections,
        };
        match change {
          Change::Limits { base, asked } => {
            project_settings::save_limits_in(&root, &runners, base, asked).map_err(|e| messages::store_error(&e))?;
          }
          Change::Files(on) => {
            project_settings::save_agent_files_in(&root, on).map_err(|e| messages::store_error(&e))?
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
          Change::UseAccount(id) => expect_active_account_in(&root, id).map_err(|e| e.message())?,
        }
        project_settings::load_in(&root)
          .map(|settings| Saved(settings, outside))
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
}

impl OwnSaves {
  pub fn new(ctx: &mut Ctx, kinds: &'static [Outside]) -> Self {
    Self {
      read: Arc::default(),
      changes: OwnChanges::new(ctx, kinds),
    }
  }

  /// How often the kinds the tab shows changed. Reading it in a render renders again when it moves.
  pub fn count(&self, feed: &ProjectFeed) -> u64 {
    self.changes.count(feed)
  }
}

pub fn load(ctx: &mut Ctx, root: PathBuf, revision: u64, own: &OwnSaves) -> FutureState<Read, Message> {
  let saved = Arc::clone(&own.read);
  let mut state = ctx
    .future((root, revision), move |(root, revision)| {
      let saved = saved.lock().take();
      if saved.is_none() {
        tracing::trace!(target: crate::app_log::READ_LOG, read = "project settings");
      }
      async move {
        if let Some(settings) = saved {
          return Ok(Read(settings, revision));
        }
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
  let changes = Services::of(ctx).changes;
  ctx.watch(&save.state(), move |state| {
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
      // Every other screen that shows the change reads it again; the tab takes what the save read, in the
      // same render, so neither its revision nor the feed catching up reads the settings again.
      own.changes.record(&changes, project.open.project.id(), saved.1);
      *own.read.lock() = Some(saved.0.clone());
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
pub fn pending(ctx: &Ctx, state: &FutureState<Read, Message>, revision: &Signal<u64>) -> Element {
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
    None => loading("settings-loading", &ctx.t("project_settings.reading")).into(),
  }
}
