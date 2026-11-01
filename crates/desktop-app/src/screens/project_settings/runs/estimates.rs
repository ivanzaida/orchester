//! Estimates (`Estimates` `vLiTy` in `02 · Runs`, the Runs window `g406O`): *Let Orchester estimate story points*, the
//! project's own setting since ORC-103 (a project a person has not set it for keeps the application-wide setting it
//! moved from). Orchester's estimator then sizes new tasks, and tasks whose brief or criteria change, on auto mode's
//! reviewer, never changing points a person set. It is read and saved off the UI thread, and its switch cannot be
//! flipped while either runs. Only a person flips it: the switch refuses lurq's synthetic input, an MCP client's
//! included, and says so under the row. Unlike the application-wide Automation section it moved from, the row does not
//! close the whole window to MCP input while it shows, so Runs' other settings stay open to automation as before.
//! While estimates are on and the estimator's last request could not run, the row says why in its hint's place, in
//! `warning` (ORC-611), instead of only in the log.

use std::{path::Path, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, FutureState},
    events::{InputSource, input_source},
  },
  core::Signal,
  node::Element,
};
use orchester_core::time::Timestamp;
use orchester_workspace::{
  background,
  messages::{self, Message},
};

use super::super::{
  TabProps,
  kit::{Group, SettingRow, Tone},
};
use crate::{
  screens::reviewer_trouble,
  services::Services,
  ui::{
    field::field_error,
    message::message_text,
    toggle::{Toggle, ToggleKind},
  },
};

/// The switch's element id.
const ID: &str = "settings-estimate-points";

type SaveAction = FutureAction<bool, bool, Message>;

pub(super) struct Estimates {
  /// What the person last chose; `None` until they do.
  saved: Signal<Option<bool>>,
  /// A change the row did not take because synthetic input brought it.
  blocked: Signal<bool>,
  save: SaveAction,
}

impl Component for Estimates {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    let workspace = Services::of(ctx).workspace;
    let root = ctx.props::<TabProps>().project.open.repo_root.clone();
    let save = ctx.future_action(move |allowed: bool| {
      let (workspace, root) = (workspace.clone(), root.clone());
      async move {
        background::run(move || workspace.set_estimates_story_points(&root, allowed))
          .await?
          .map(|()| allowed)
          .map_err(|error| {
            tracing::error!(%error, "Let Orchester estimate story points was not saved");
            messages::store_error(&error)
          })
      }
    });
    Self {
      saved: ctx.signal(None),
      blocked: ctx.signal(false),
      save,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let state = self.save.state().get();
    let read = self.stored(ctx, state.data);
    let root = ctx.props::<TabProps>().project.open.repo_root.clone();
    let fixed = state.is_pending() || read.is_pending();
    // The person's choice once it saved; the stored value until then, or after a save failed.
    let shown = match (&state.error, self.saved.get()) {
      (None, Some(chosen)) => Some(chosen),
      _ => read.data,
    };
    let switch = Toggle::new(
      ID,
      ToggleKind::Switch,
      shown.unwrap_or(false),
      ctx.t("project_settings.estimates.label"),
    )
    .hide_label();
    let switch = if shown.is_some() && !fixed {
      self.listen(switch)
    } else {
      switch
    };
    let problem = match (&read.error, &state.error) {
      (Some(reason), _) => Some(("project_settings.estimates.unreadable", reason)),
      (None, Some(reason)) => Some(("project_settings.estimates.notSaved", reason)),
      (None, None) => None,
    }
    .map(|(key, reason)| {
      let reason = message_text(ctx, reason);
      Element::from(field_error(
        &format!("{ID}-error"),
        &ctx.t_args(key, [("reason", &*reason)]),
      ))
    });
    let blocked = self.blocked.get().then(|| {
      Element::from(field_error(
        &format!("{ID}-blocked"),
        &ctx.t("project_settings.estimates.blocked"),
      ))
    });
    let row = SettingRow::new(ID, ctx.t("project_settings.estimates.label"));
    // Why the last estimate gave no points, in the hint's place while estimates are on.
    let row = match could_not_run(ctx, &root, shown) {
      Some(said) => row.toned(said, Tone::Warning),
      None => row.hint(ctx.t("project_settings.estimates.hint")),
    };
    let row = row
      .info(ctx.t("project_settings.estimates.info"))
      .control(switch)
      .under_all(problem)
      .under_all(blocked)
      .render(ctx);
    Element::from(Group::new("settings-estimates", ctx.t("project_settings.estimates.title")).row(row))
  }
}

impl Estimates {
  /// `switch`, saving what a person chooses; synthetic input never changes it.
  fn listen(&self, switch: Toggle) -> Toggle {
    let (saved, blocked, save) = (self.saved.clone(), self.blocked.clone(), self.save.clone());
    switch.on_change(move |chosen| {
      if input_source() == InputSource::Synthetic {
        tracing::warn!("synthetic input reached Let Orchester estimate story points; not taken");
        blocked.set(true);
        return;
      }
      blocked.set(false);
      saved.set(Some(chosen));
      save.run(chosen);
    })
  }

  /// The project's setting as stored, read when the row shows and again after each save (`saved`).
  fn stored(&self, ctx: &mut Ctx, saved: Option<bool>) -> FutureState<bool, Message> {
    let workspace = Services::of(ctx).workspace;
    let root = ctx.props::<TabProps>().project.open.repo_root.clone();
    ctx
      .future(saved, move |_| {
        let (workspace, root) = (workspace.clone(), root.clone());
        async move {
          background::run(move || workspace.estimates_story_points(&root))
            .await?
            .map_err(|error| {
              tracing::error!(%error, "Let Orchester estimate story points cannot be read");
              messages::store_error(&error)
            })
        }
      })
      .state()
      .get()
  }
}

/// Why the estimator's last request on the reviewer gave no points (ORC-611), while estimates are on (`shown`); read
/// when the row shows and again as the switch changes.
fn could_not_run(ctx: &mut Ctx, root: &Path, shown: Option<bool>) -> Option<Arc<str>> {
  let health = reviewer_trouble::health(ctx, root.to_path_buf(), format!("{shown:?}"));
  let now = Timestamp::now();
  let trouble = health.filter(|_| shown == Some(true))?.estimates(now)?;
  let reason = reviewer_trouble::reason(ctx, trouble, now);
  Some(ctx.t_args("project_settings.estimates.couldNotRun", [("reason", &*reason)]))
}
