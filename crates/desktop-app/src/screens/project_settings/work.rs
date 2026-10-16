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
};

use super::{Status, TabProps};
use crate::{
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved(pub Settings);
impl DevtoolsInspectable for Saved {}
pub type Save = FutureAction<(PathBuf, Change), Saved, Message>;

pub fn action(ctx: &mut Ctx) -> Save {
  let services = Services::of(ctx);
  ctx.future_action(move |(root, change): (PathBuf, Change)| {
    let (runners, changes) = (services.runners.clone(), services.changes.clone());
    async move {
      background::run(move || {
        let outside = match &change {
          Change::Limits { .. } => orchester_workspace::tasks::outside::Outside::Limits,
          Change::Auto { .. } => orchester_workspace::tasks::outside::Outside::AutoMode,
          Change::Index(_) => orchester_workspace::tasks::outside::Outside::CodeIndex,
          Change::Files(_) => orchester_workspace::tasks::outside::Outside::ProjectSettings,
          Change::UseAccount(_) => orchester_workspace::tasks::outside::Outside::Connections,
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
        let db = orchester_store::ProjectDb::open(&root).map_err(|e| messages::store_error(&e))?;
        changes.record(db.project().id(), outside);
        project_settings::load_in(&root)
          .map(Saved)
          .map_err(|e| messages::store_error(&e))
      })
      .await?
    }
  })
}
pub fn load(ctx: &mut Ctx, root: PathBuf, revision: u64) -> FutureState<Read, Message> {
  let mut state = ctx
    .future((root, revision), |(root, revision)| {
      tracing::trace!(target: crate::app_log::READ_LOG, read = "project settings");
      async move {
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
pub fn follow(ctx: &mut Ctx, save: &Save, status: Signal<Status>, revision: Signal<u64>, stored: Signal<Option<Read>>) {
  let project = ctx.props::<TabProps>().project.clone();
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
