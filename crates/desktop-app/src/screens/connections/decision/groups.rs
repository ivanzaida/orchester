//! Frame 02's grouped Server, Models and Used by rows.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::Row,
  layout::Alignment,
  node::Element,
};
use orchester_core::{connection::DecisionConnection, run::review::default_decision_threshold};

use super::{DecisionPage, Props, read::Read};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route},
  screens::project_settings::kit::{Group, SettingRow, mono_value, value},
  theme::IconSize,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

pub(super) fn server(ctx: &mut Ctx, read: &Read, connection: &DecisionConnection) -> Element {
  let kind = SettingRow::new("decision-kind", ctx.t("decision_connection.kind"))
    .hint(ctx.t("decision_connection.kindHint"))
    .control(value("decision-api", &ctx.t("decision_connection.api")))
    .render(ctx);
  let url = SettingRow::new("decision-url", ctx.t("decision_connection.baseUrl"))
    .control(mono_value("decision-url-value", connection.base_url().as_str()))
    .render(ctx);
  let key = read.key_name.clone().unwrap_or_else(|| {
    ctx
      .t(if connection.key().is_some() {
        "decision_connection.keyMissing"
      } else {
        "decision_connection.noKey"
      })
      .to_string()
  });
  let key = SettingRow::new("decision-key", ctx.t("decision_connection.apiKey"))
    .hint(ctx.t("decision_connection.keyHint"))
    .control(mono_value("decision-key-name", &key))
    .render(ctx);
  let local = connection.is_on_this_computer();
  let where_to = if local {
    ctx.t("decision_connection.thisComputer")
  } else {
    connection.base_url().host().into()
  };
  let hint = ctx.t(if local {
    "decision_connection.localRequests"
  } else {
    "decision_connection.remoteRequests"
  });
  let where_to = Row::new()
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(icon(
      if local { Icon::Monitor } else { Icon::Send },
      IconSize::Sm,
      if local {
        PaletteColor::Success
      } else {
        PaletteColor::Warning
      },
    ))
    .child(value("decision-destination-value", &where_to));
  let requests = SettingRow::new("decision-destination", ctx.t("decision_connection.requestsGoTo"))
    .hint(hint)
    .control(where_to)
    .render(ctx);
  Group::new("decision-server", ctx.t("decision_connection.server"))
    .rows([kind, url, key, requests])
    .into()
}

pub(super) fn models(
  ctx: &mut Ctx,
  page: &DecisionPage,
  props: &Props,
  read: &Read,
  connection: &DecisionConnection,
) -> Element {
  let models = connection
    .models()
    .iter()
    .map(|model| model.as_str())
    .collect::<Vec<_>>()
    .join(", ");
  let models = if models.is_empty() {
    ctx.t("decision_connection.noModels")
  } else {
    models.into()
  };
  let refresh = page
    .check_button(
      ctx,
      props,
      read,
      connection,
      ("decision-refresh", "decision_connection.refresh"),
    )
    .compact();
  let row = SettingRow::new("decision-models-listed", ctx.t("decision_connection.modelsListed"))
    .hint(models)
    .control(refresh)
    .render(ctx);
  Group::new("decision-models", ctx.t("decision_connection.models"))
    .row(row)
    .into()
}

pub(super) fn used_by(ctx: &mut Ctx, props: &Props, read: &Read, connection: &DecisionConnection) -> Element {
  let row = if let Some(reviewer) = &read.reviewer {
    let threshold = reviewer.threshold.unwrap_or_else(default_decision_threshold).value();
    let hint = ctx.t_args(
      "decision_connection.reviewerHint",
      [
        ("model", reviewer.model.as_str().to_owned()),
        ("threshold", format!("{threshold:.2}")),
      ],
    );
    let open = Navigation::of(ctx).to(Route::Project(
      props.project.open.project.id(),
      ProjectPage::Settings(ProjectSettingsTab::Runs),
    ));
    let action = ActionButton::new(
      "decision-open-runs",
      ButtonKind::Ghost,
      ctx.t("decision_connection.runs"),
    )
    .icon(Icon::ArrowUpRight)
    .compact()
    .opens(open);
    let missing = !connection.models().contains(&reviewer.model);
    let row = SettingRow::new("decision-used-reviewer", ctx.t("decision_connection.reviewer"))
      .hint(hint)
      .control(action);
    let row = if missing {
      row.lines([crate::screens::project_settings::kit::form_note(
        "decision-reviewer-missing",
        &ctx.t("decision_connection.reviewerMissing"),
      )
      .into()])
    } else {
      row
    };
    row.render(ctx)
  } else {
    SettingRow::new("decision-unused", ctx.t("decision_connection.unused")).render(ctx)
  };
  Group::new("decision-used-by", ctx.t("decision_connection.usedBy"))
    .row(row)
    .into()
}
