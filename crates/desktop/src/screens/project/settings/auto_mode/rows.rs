//! The rows of the Auto mode section: *New runs use auto mode*, and *Reviewer* with its connection, model
//! and effort, each labelled as an agent profile's fields are (*Project connection*, *Model*, *Reasoning
//! effort*), then which model reviews or why none does. A connection that cannot review is listed but
//! disabled, with the reason under it.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Select, SelectOption},
  core::SignalValue,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentConnection, ModelSupport},
  resource::ModelId,
  run::review::{AutoModeSettings, resolve_reviewer},
};

use super::{
  super::{
    super::task::auto_mode::reviewed_by,
    general::{readback, sized_setting_row},
  },
  AutoModeSection, Chosen, Commit, ConnectionPick, EffortPick,
  choice::{Stored, cannot_review},
};
use crate::{
  agents::form::{effort_options, model_options},
  tasks::run_approvals::{ReviewerShown, unavailable_key},
  ui::{
    field::{FieldNote, field, select_style},
    switch::{Switch, locked_reason},
  },
};

/// A setting's value column (`LmaBU`) and a select's height (`bhw3I`).
const VALUE_WIDTH: f32 = 300.0;
const SELECT_HEIGHT: f32 = 32.0;

impl AutoModeSection {
  /// *Reviewer*: the connection, its model and effort, and which model reviews or why none does.
  pub(super) fn reviewer_row(&self, ctx: &Ctx, stored: &Stored) -> Element {
    // Without an agent connection there is nothing to choose: the row says why auto mode has no reviewer.
    let mut value: Vec<Element> = (!stored.connections.is_empty())
      .then(|| self.connection_select(ctx, stored))
      .into_iter()
      .collect();
    match &resolve_reviewer(&stored.settings, &stored.connections) {
      Ok(reviewer) => {
        if let Ok(agent) = AgentConnection::try_from(reviewer.connection.clone()) {
          value.push(self.model_select(ctx, &agent, &reviewer.model));
          value.extend(self.effort_select(ctx, &agent, &reviewer.model));
        }
        let shown = ReviewerShown {
          connection: reviewer.connection.name().as_str().into(),
          model: reviewer.model.clone(),
          effort: reviewer.effort.clone(),
        };
        value.push(readback(&reviewed_by(ctx, &shown)).id("settings-auto-mode-reviewer"));
      }
      Err(why) => value.push(locked_reason(
        "settings-auto-mode-unavailable".to_owned(),
        &ctx.t(unavailable_key(*why)),
      )),
    }
    sized_setting_row(
      VALUE_WIDTH,
      &ctx.t("settings.autoMode.reviewerName"),
      &ctx.t("settings.autoMode.reviewerDescription"),
      value,
    )
  }

  /// *Project connection*: the default reviewer first, then the project's agent connections; one that
  /// cannot review is disabled, with the reason.
  fn connection_select(&self, ctx: &Ctx, stored: &Stored) -> Element {
    let default = resolve_reviewer(
      &AutoModeSettings {
        reviewer: None,
        ..stored.settings.clone()
      },
      &stored.connections,
    )
    .map_or_else(
      |_| ctx.t("settings.autoMode.defaultNone"),
      |reviewer| {
        ctx.t_args(
          "settings.autoMode.defaultConnection",
          [("connection", reviewer.connection.name().as_str())],
        )
      },
    );
    let mut options = vec![SelectOption::new(ConnectionPick(None), default)];
    for connection in &stored.connections {
      let option = SelectOption::new(ConnectionPick(Some(connection.id())), connection.name().as_str());
      options.push(match cannot_review(connection) {
        Some(why) => option.detail(ctx.t(unavailable_key(why))).disabled(true),
        None => option,
      });
    }
    if let Some(choice) = &stored.settings.reviewer
      && !stored
        .connections
        .iter()
        .any(|connection| connection.id() == choice.connection)
    {
      options.push(SelectOption::new(
        ConnectionPick(Some(choice.connection)),
        ctx.t("settings.autoMode.removedConnection"),
      ));
    }
    let select = select(
      ctx,
      Select::new(self.picks.connection.clone()).options(options),
      "settings-auto-mode-connection",
    );
    field(&ctx.t("agents.connection"), select, FieldNote::None)
  }

  /// *Model*: the reviewer connection's models, as an agent profile's model field lists them.
  fn model_select(&self, ctx: &Ctx, connection: &AgentConnection, current: &ModelId) -> Element {
    let options: Vec<(Chosen, Arc<str>)> = model_options(connection, Some(current))
      .into_iter()
      .map(|option| {
        let label = match option.support {
          ModelSupport::Listed => option.id.as_str().into(),
          ModelSupport::NotVerified => ctx.t_args("agents.modelNotVerified", [("model", option.id.as_str())]),
        };
        (Chosen(Some(option.id)), label)
      })
      .collect();
    let select = select(
      ctx,
      Select::new(self.picks.model.clone()).options(options),
      "settings-auto-mode-model",
    );
    field(&ctx.t("agents.model"), select, FieldNote::None)
  }

  /// *Reasoning effort*, when the model has efforts.
  fn effort_select(&self, ctx: &Ctx, connection: &AgentConnection, model: &ModelId) -> Option<Element> {
    let efforts = effort_options(connection, model);
    if efforts.is_empty() {
      return None;
    }
    let options = std::iter::once((EffortPick(None), ctx.t("agents.effortDefault"))).chain(
      efforts
        .iter()
        .map(|effort| (EffortPick(Some(effort.clone())), effort.as_str().into())),
    );
    let select = select(
      ctx,
      Select::new(self.picks.effort.clone()).options(options),
      "settings-auto-mode-effort",
    );
    Some(field(&ctx.t("agents.effort"), select, FieldNote::None))
  }
}

/// `select` at the settings' select height, `id` for tooling.
fn select<T>(ctx: &Ctx, select: Select<T>, id: &'static str) -> Element
where
  T: SignalValue + Clone + PartialEq + Send + Sync + 'static,
  Vec<T>: SignalValue,
{
  Element::from(
    select
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(SELECT_HEIGHT))
      .style(select_style(ctx, SELECT_HEIGHT)),
  )
  .id(id)
}

/// *New runs use auto mode*, which saves when it flips; fixed while auto mode has no reviewer.
pub(super) fn new_runs_row(ctx: &Ctx, stored: &Stored, commit: &Commit) -> Element {
  let on = stored.settings.new_runs;
  let state = ctx.t(if on {
    "settings.autoMode.on"
  } else {
    "settings.autoMode.off"
  });
  let switch = Switch::new("settings-auto-mode-new-runs", state, on).emphasized();
  let switch = if stored.resolved().is_some() {
    let commit = commit.clone();
    switch.render(move || {
      commit.change(&|current| {
        Ok(Some(AutoModeSettings {
          new_runs: !current.settings.new_runs,
          ..current.settings.clone()
        }))
      });
    })
  } else {
    switch.locked()
  };
  sized_setting_row(
    VALUE_WIDTH,
    &ctx.t("settings.autoMode.newRunsName"),
    &ctx.t("settings.autoMode.newRunsDescription"),
    vec![switch],
  )
}
