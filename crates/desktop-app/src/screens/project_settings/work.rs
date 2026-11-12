//! Off-thread saves using workspace services, and the page's read/save feedback. The tabs read the project's settings
//! through their query (`queries::project::project_settings`), which the project's loader read as the project opened.
//! A save reads the settings back right after writing them, which the tab shows at once, and records its change, which
//! reads the query again (`queries`); what the save read shows until the query has it.
use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction, FutureState},
  },
  core::Signal,
  node::Element,
  query::QueryHandle,
};
use orchester_core::{
  id::{ConnectionId, ProjectId},
  limits::ProjectLimits,
  run::{Answerer, ApproveMode, ProjectRuleId, review::AutoModeSettings},
};
use orchester_workspace::{
  background, code_index,
  connections::github::expect_active_account_in,
  messages::{self, Message},
  project_settings::{self, Settings},
  tasks::outside::Outside,
};

use super::{Status, TabProps};
use crate::{
  queries::{self, Kept, Session, project},
  services::Services,
  ui::{field::field_error, message::message_text, skeleton::Skeleton, state::load_error},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read(pub Settings);
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
    decision: Option<Box<orchester_core::connection::DecisionConnection>>,
  },
  /// The project's *Approve mode*, moved up by a person (to Dangerous auto-approve through its confirmation), or
  /// down by anyone.
  Mode {
    to: ApproveMode,
    by: Answerer,
  },
  UseAccount(ConnectionId),
  /// Revokes a rule of the project's (*Approve for every run of this project*).
  Revoke(ProjectRuleId),
}
/// The settings as the save read them after writing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved(pub Settings);
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
          Change::Auto { .. } | Change::Mode { .. } | Change::Revoke(_) => Outside::AutoMode,
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
          Change::Auto {
            base,
            asked,
            confirmed,
            decision,
          } => project_settings::save_auto_mode_in(&root, &base, &asked, confirmed, decision.as_deref())?,
          Change::Mode { to, by } => project_settings::save_approve_mode_in(&root, to, by).map(|_| ())?,
          Change::UseAccount(id) => expect_active_account_in(&root, id).map_err(|e| e.message())?,
          Change::Revoke(rule) => {
            project_settings::revoke_project_rule_in(&root, rule).map_err(|e| messages::store_error(&e))?
          }
        }
        // Right after the write, whether or not the tab is still there to take the save: every screen that
        // shows the change reads it again, and so does the settings' query.
        changes.record(project, outside);
        project_settings::load_in(&root)
          .map(Saved)
          .map_err(|e| messages::store_error(&e))
      })
      .await?
    }
  })
}
/// The project's settings as its query has them, and the query, which *Try again* reads again.
pub struct Loaded {
  pub state: FutureState<Read, Message>,
  query: QueryHandle<Settings, Message>,
}

impl Loaded {
  /// What a tab shows: what its own last save read back, until the query has it or read again since (a failed save
  /// kept nothing), else the query's.
  pub fn shown(&self, kept: &Kept<Read>, saving: bool) -> Option<Read> {
    kept.over((self.state.data.clone(), &self.query), saving)
  }
}

/// The settings of the project at `root`, from the session's cache: its last read in the first frame, read again
/// behind it when a change invalidated it.
pub fn load(ctx: &mut Ctx, root: PathBuf) -> Loaded {
  let session = Session::of(ctx);
  let query = ctx.query(project::project_settings(session, root));
  let FutureState { status, data, error } = queries::state(&query);
  Loaded {
    state: FutureState {
      status,
      data: data.map(Read),
      error,
    },
    query,
  }
}

/// Follows the tab's `save`: the page's `status`, and what the save read back, `kept` until the query moves on.
pub fn follow(ctx: &mut Ctx, save: &Save, status: Signal<Status>, kept: Kept<Read>) {
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
      kept.keep(Read(saved.0.clone()));
      status.set(Status::Saved);
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
/// What a tab shows before its settings were ever read: why reading failed with Try again, or the `bones` it builds in
/// its skeleton, named *Reading project settings…* (`project_settings.reading`).
pub fn pending<T: Into<Element>>(ctx: &mut Ctx, loaded: &Loaded, bones: impl FnOnce(&mut Ctx) -> T) -> Element {
  match &loaded.state.error {
    Some(error) => {
      let query = loaded.query.clone();
      load_error(
        ctx,
        "settings-load-error",
        (&ctx.t("project_settings.loadFailed"), &message_text(ctx, error)),
        move || query.refresh(),
      )
    }
    None => {
      let bones = bones(ctx);
      Skeleton::new("settings-loading", ctx.t("project_settings.reading")).show(ctx, bones)
    }
  }
}
