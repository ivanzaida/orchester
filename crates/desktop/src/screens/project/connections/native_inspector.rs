//! A native connection's details in the inspector (04.H4 `lJG42`, statuses 04.H5 `dtxII`): provider,
//! preset and endpoint in full, where its data goes, its key secret, how it runs and its model traffic,
//! the balance and the last test, then its models with their source, limits and efforts. A status that
//! needs the key fixed says how, with the way there.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentConnection, ApiEndpoint, ConnectionStatus, NativeModel},
  secret::Secret,
  time::Timestamp,
};

use super::{
  inspector::{Handler, InspectorActions, button, link_button},
  model_text,
  status::{preset_key, program_key},
};
use crate::{
  display,
  screens::project::secrets::labels::environment_key,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
    property::{ValueStyle, property, styled_property, wrapping_mono_property},
    status_label::compact_status_label,
  },
};

/// Provider, preset, endpoint, where the data goes, key, execution, model traffic, balance and the last
/// test.
pub fn properties(ctx: &Ctx, connection: &AgentConnection, api: &ApiEndpoint, secrets: &[Secret]) -> Vec<Element> {
  let host = api.base_url().host();
  let mut rows = vec![
    property(
      &ctx.t("connections.property.provider"),
      &ctx.t(program_key(connection.provider())),
    ),
    property(
      &ctx.t("connections.native.property.preset"),
      &ctx.t(preset_key(api.preset())),
    ),
    wrapping_mono_property(&ctx.t("connections.property.endpoint"), api.base_url().as_str()),
    data_line(
      Icon::Globe,
      &ctx.t_args("connections.dataNotice.sent", [("host", host)]),
    ),
  ];
  if api.preset().has_data_notice() {
    rows.push(data_line(Icon::Database, &ctx.t("connections.dataNotice.storedInPrc")));
  }
  let checked = connection.last_check();
  rows.extend([
    property(
      &ctx.t("connections.property.authentication"),
      &key_text(ctx, api, secrets),
    ),
    property(
      &ctx.t("connections.property.execution"),
      &ctx.t("connections.native.execution"),
    ),
    property(
      &ctx.t("connections.native.property.modelTraffic"),
      &ctx.t("connections.native.modelTraffic"),
    ),
  ]);
  rows.extend(
    checked
      .and_then(|check| check.result.balance())
      .map(|balance| styled_property(&ctx.t("connections.native.property.balance"), balance, ValueStyle::Mono)),
  );
  let last = checked.map_or_else(
    || ctx.t("connections.notCheckedYet"),
    |check| message_text(ctx, &display::elapsed(Timestamp::now(), check.at)),
  );
  rows.push(property(&ctx.t("connections.property.lastChecked"), &last));
  rows
}

/// *API key · NAME (Environment)*, or that the key secret is gone.
pub fn key_text(ctx: &Ctx, api: &ApiEndpoint, secrets: &[Secret]) -> Arc<str> {
  match secrets.iter().find(|secret| secret.id() == api.key()) {
    Some(secret) => ctx.t_args(
      "connections.native.key.authentication",
      [
        ("name", secret.name().as_str()),
        ("environment", &*ctx.t(environment_key(secret.environment()))),
      ],
    ),
    None => ctx.t("connections.native.key.gone"),
  }
}

/// Where the connection's data goes (`q5LzI`, globe): an icon beside a wrapping caption.
fn data_line(glyph: Icon, text: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Small, PaletteColor::TextSecondary))
    .child(Text::new(text).variant(ExtraTypography::CellLabel.role()).flex(1.0))
    .into()
}

/// The models (`o5BqGF`), each with its default label and what is known of it, then editing them by
/// hand and fetching them from the endpoint, which needs a test this version cannot run.
pub fn models(ctx: &Ctx, api: &ApiEndpoint, edit_models: Option<Handler>) -> Element {
  let models = api.models();
  let column = Column::new()
    .id("native-models")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .child(Text::new(&ctx.t("connections.models.title")).variant(TypographyStyle::Label));
  let column = if models.is_empty() {
    column.child(Text::new(&ctx.t("connections.models.none")).variant(TypographyStyle::Caption))
  } else {
    column.with_children(
      models
        .models()
        .iter()
        .map(|model| model_row(ctx, model, models.default_model() == Some(model.id()))),
    )
  };
  column
    .child(button(
      ctx,
      "edit-models",
      "connections.models.edit",
      Icon::Pencil,
      edit_models,
    ))
    .child(button(
      ctx,
      "fetch-models",
      "connections.models.fetch",
      Icon::RefreshCw,
      None,
    ))
    .into()
}

/// A model's id, *default* when profiles that choose none run it, and its source, limits and efforts.
fn model_row(ctx: &Ctx, model: &NativeModel, default: bool) -> Element {
  let id = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(model.id().as_str())
        .variant(ExtraTypography::MonoValue.role())
        .flex(1.0),
    )
    .with_children(default.then(|| compact_status_label(Icon::Check, &ctx.t("connections.models.default"))));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(id)
    .child(Text::new(&model_text::summary(ctx, model)).variant(ExtraTypography::InspectorNote.role()))
    .into()
}

/// What a key status offers (04.H5 `dtxII`): choosing another secret and opening Secrets when the key
/// is missing, replacing its value in Secrets when it was rejected; `None` for other statuses.
pub fn status_actions(ctx: &Ctx, status: ConnectionStatus, actions: &InspectorActions) -> Option<Element> {
  let buttons = match status {
    ConnectionStatus::KeyMissing => vec![
      button(
        ctx,
        "choose-key-secret",
        "connections.native.key.choose",
        Icon::KeyRound,
        actions.edit.clone(),
      ),
      link_button(
        ctx,
        "open-secrets",
        "connections.native.key.openSecrets",
        Icon::ExternalLink,
        (actions.secrets)(),
      ),
    ],
    ConnectionStatus::KeyRejected => vec![link_button(
      ctx,
      "replace-key-secret",
      "connections.native.key.replace",
      Icon::ExternalLink,
      (actions.secrets)(),
    )],
    _ => return None,
  };
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Step.role())
      .with_children(buttons)
      .into(),
  )
}
