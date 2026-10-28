//! A native connection's page (`05b` `PGwka`, adapting the old design's native inspector 04.H4–H8): its status and
//! what it asks for, *Test connection* through the project's runner, which reads the key and lists the model API's
//! models without spending tokens, and what the test found in place; its endpoint, key and execution with where
//! requests go; its models with where each came from, *Fetch models* and *Edit models*; what Orchester enforces for its
//! agents ([`super::enforced`]); and the profiles that use it. While the key's secret is missing, Edit connection leads
//! the head's actions and the test waits for a key.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{
  connection::{
    AgentConnection, ApiEndpoint, ConnectionStatus, EndpointPreset, ModelSource, NativeModel, NativeModels,
    offers_provider_fallback,
  },
  secret::Secret,
  tokens::compact_tokens,
};
use orchester_workspace::display;

use super::{
  Actions, Inspecting, Parts,
  agent::{checked_detail, menu},
  check_button, detail_row, enforced,
  rename::title_with_rename,
  result, sentence, sentence_tone, shown,
};
use crate::{
  screens::{
    connections::{
      provider_form::environment_key,
      words::{agent_glyph, connection_status, provider_key},
    },
    project_settings::kit::{Group, SettingRow, Tone, value, warning_line},
  },
  theme::ExtraTypography,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
  },
};

pub(super) fn parts(
  ctx: &mut Ctx,
  (connection, api): (&AgentConnection, &ApiEndpoint),
  used: Element,
  (inspecting, actions): (&Inspecting<'_>, &Actions),
) -> Parts {
  let status = connection.status_with(&inspecting.listed.secrets);
  let shown_status = shown(inspecting, connection_status(connection, status));
  let test = check_button(inspecting, ("connection-test", ctx.t("connections.test")), actions);
  let head_actions = if status == ConnectionStatus::KeyMissing {
    let edit = ActionButton::new("connection-edit-key", ButtonKind::Primary, ctx.t("connections.edit"))
      .icon(Icon::Pencil)
      .opens((actions.edit)(connection.id()));
    vec![
      edit.into(),
      test.unavailable(ctx.t("connections.native.testNeedsKey")).into(),
    ]
  } else {
    vec![test.into()]
  };
  let mut notices: Vec<Element> = detail_key(status)
    .map(|key| {
      sentence(
        "connection-inspector-sentence",
        &ctx.t(key),
        sentence_tone(shown_status),
      )
    })
    .into_iter()
    .collect();
  notices.extend(result::native(ctx, connection, api, inspecting));
  let connection_group = Group::new("connection-properties", ctx.t("connections.connectionGroup"))
    .rows(rows(ctx, connection, api, inspecting))
    .after(sent_to(ctx, api));
  Parts {
    glyph: agent_glyph(connection),
    title: title_with_rename(
      ctx,
      (inspecting, actions),
      (connection.id(), connection.name().as_str()),
    ),
    status: shown_status,
    status_detail: checked_detail(ctx, connection, inspecting),
    actions: head_actions,
    menu: menu(ctx, connection, (inspecting, actions)),
    notices,
    groups: vec![
      connection_group.into(),
      models(ctx, (connection, api), inspecting, actions),
      enforced::group(ctx, connection.provider()),
      used,
    ],
    question: None,
  }
}

/// What a status means and what to do about it; nothing once the API answered and took the key.
const fn detail_key(status: ConnectionStatus) -> Option<&'static str> {
  match status {
    ConnectionStatus::Connected => None,
    ConnectionStatus::KeyMissing => Some("connections.native.detail.keyMissing"),
    ConnectionStatus::KeyRejected => Some("connections.native.detail.keyRejected"),
    ConnectionStatus::BillingProblem => Some("connections.native.detail.billing"),
    ConnectionStatus::EndpointUnreachable => Some("connections.native.detail.unreachable"),
    ConnectionStatus::NotChecked
    | ConnectionStatus::SignInRequired
    | ConnectionStatus::SignInUnknown
    | ConnectionStatus::ExecutableMissing
    | ConnectionStatus::NotRecognized
    | ConnectionStatus::CouldNotRun => Some("connections.native.detail.notTested"),
  }
}

/// The provider, preset, endpoint, the key's secret by name (never its value), where the agent runs and whether a
/// declined request is retried on another model.
fn rows(ctx: &mut Ctx, connection: &AgentConnection, api: &ApiEndpoint, inspecting: &Inspecting<'_>) -> Vec<Element> {
  let provider = ctx.t(provider_key(connection.provider()));
  let preset = ctx.t(preset_key(api.preset()));
  let mut rows = vec![
    detail_row(
      ctx,
      "connection-provider",
      (&ctx.t("connections.property.provider"), &provider),
      false,
    ),
    detail_row(
      ctx,
      "connection-preset",
      (&ctx.t("connections.property.preset"), &preset),
      false,
    ),
    detail_row(
      ctx,
      "connection-endpoint",
      (&ctx.t("connections.property.endpoint"), api.base_url().as_str()),
      true,
    ),
  ];
  rows.push(key_row(ctx, api, &inspecting.listed.secrets));
  let execution = ctx.t("connections.native.execution");
  rows.push(detail_row(
    ctx,
    "connection-execution",
    (&ctx.t("connections.property.execution"), &execution),
    false,
  ));
  let fallback = ctx.t(if !offers_provider_fallback(connection.provider(), api.preset()) {
    "connections.native.fallback.notOffered"
  } else if api.provider_fallback() {
    "connections.native.fallback.on"
  } else {
    "connections.native.fallback.off"
  });
  rows.push(
    SettingRow::new("connection-fallback", ctx.t("connections.native.fallback.label"))
      .hint(ctx.t("connections.native.fallback.hint"))
      .control(value("connection-fallback-value", &fallback))
      .render(ctx),
  );
  rows
}

/// The key by its secret's name and environment, or that the secret is archived or gone.
fn key_row(ctx: &mut Ctx, api: &ApiEndpoint, secrets: &[Secret]) -> Element {
  let (words, tone) = match secrets.iter().find(|secret| secret.id() == api.key()) {
    Some(secret) => (
      ctx.t_args(
        "connections.native.key",
        [
          ("name", secret.name().as_str()),
          ("environment", &*ctx.t(environment_key(secret.environment()))),
        ],
      ),
      Tone::Muted,
    ),
    None => (ctx.t("connections.native.keyGone"), Tone::Danger),
  };
  SettingRow::new(
    "connection-authentication",
    ctx.t("connections.property.authentication"),
  )
  .hint(ctx.t("connections.native.keyHint"))
  .control(
    value("connection-authentication-value", &words)
      .variant(ExtraTypography::PropertyMono.role())
      .color(tone.color()),
  )
  .render(ctx)
}

/// Where the agents' requests go, under the connection's rows (`Sent to` `s4sjZm`).
fn sent_to(ctx: &mut Ctx, api: &ApiEndpoint) -> Element {
  let words = ctx.t_args("connections.native.sentTo", [("host", api.base_url().host())]);
  warning_line(
    ctx,
    "connection-native-sent-to",
    &words,
    Some(ctx.t("connections.native.sentToInfo")),
  )
}

/// *Models*: each model the connection offers with where it came from, its limits and efforts, the default marked;
/// *Fetch models* through the runner and *Edit models* at the heading.
fn models(
  ctx: &mut Ctx,
  (connection, api): (&AgentConnection, &ApiEndpoint),
  inspecting: &Inspecting<'_>,
  actions: &Actions,
) -> Element {
  let fetch = check_button(
    inspecting,
    ("connection-fetch-models", ctx.t("connections.native.fetchModels")),
    actions,
  )
  .compact();
  let fetch = match connection.status_with(&inspecting.listed.secrets) {
    ConnectionStatus::KeyMissing => fetch.unavailable(ctx.t("connections.native.testNeedsKey")),
    _ => fetch,
  };
  let edit = ActionButton::new(
    "connection-edit-models",
    ButtonKind::Ghost,
    ctx.t("connections.native.editModels"),
  )
  .icon(Icon::Pencil)
  .compact()
  .opens((actions.edit)(connection.id()));
  let listed = api.models();
  let rows: Vec<Element> = match listed.models() {
    [] => vec![
      SettingRow::new("connection-models-none", ctx.t("connections.native.noModels"))
        .hint(ctx.t("connections.native.noModelsHint"))
        .render(ctx),
    ],
    models => models
      .iter()
      .enumerate()
      .map(|(index, model)| model_row(ctx, (index, model), listed, inspecting))
      .collect(),
  };
  Group::new("connection-models-section", ctx.t("connections.models"))
    .end(edit)
    .end(fetch)
    .rows(rows)
    .into()
}

/// One model: its id, where it came from and what is known of it; *Default* at its end, with the default effort, when
/// profiles that choose none run on it.
fn model_row(
  ctx: &mut Ctx,
  (index, model): (usize, &NativeModel),
  listed: &NativeModels,
  inspecting: &Inspecting<'_>,
) -> Element {
  let row = SettingRow::new(format!("connection-model-{index}"), model.id().as_str())
    .mono_label()
    .hint(model_detail(ctx, model, inspecting));
  if listed.default_model() != Some(model.id()) {
    return row.render(ctx);
  }
  let default = match listed.default_effort() {
    Some(effort) => ctx.t_args("connections.native.defaultWithEffort", [("effort", effort.as_str())]),
    None => ctx.t("connections.native.defaultModel"),
  };
  row
    .control(value(&format!("connection-model-{index}-default"), &default))
    .render(ctx)
}

/// Where a model came from, its context window and output cap when known, and the efforts it takes.
fn model_detail(ctx: &Ctx, model: &NativeModel, inspecting: &Inspecting<'_>) -> Arc<str> {
  let mut detail = match model.source() {
    ModelSource::Endpoint { at } => ctx.t_args(
      "connections.native.source.endpoint",
      [("time", &*message_text(ctx, &display::elapsed(inspecting.now, at)))],
    ),
    ModelSource::Preset => ctx.t("connections.native.source.preset"),
    ModelSource::Manual => ctx.t("connections.native.source.manual"),
  };
  if let (Some(context), Some(output)) = (model.context_window(), model.max_output()) {
    let limits = ctx.t_args(
      "connections.native.limits",
      [
        ("context", compact_tokens(u64::from(context.get()))),
        ("output", compact_tokens(u64::from(output.get()))),
      ],
    );
    detail = ctx.t_args("connections.native.detailJoin", [("first", detail), ("second", limits)]);
  }
  if !model.efforts().is_empty() {
    let efforts: Vec<&str> = model.efforts().iter().map(|effort| effort.as_str()).collect();
    let efforts = ctx.t_args("connections.native.efforts", [("efforts", efforts.join(", "))]);
    detail = ctx.t_args(
      "connections.native.detailJoin",
      [("first", detail), ("second", efforts)],
    );
  }
  detail
}

const fn preset_key(preset: EndpointPreset) -> &'static str {
  match preset {
    EndpointPreset::Official => "connections.native.preset.official",
    EndpointPreset::DeepSeek => "connections.native.preset.deepseek",
    EndpointPreset::Custom => "connections.native.preset.custom",
  }
}
