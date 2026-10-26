//! The page of an agent CLI connection on an API provider (`docs/design/api-providers.md`; `DeepSeek` in `05`): *Runs
//! on* with its program, the API its CLI speaks, the base URL with where requests go, and the key by its secret's name
//! (never its value); the models typed for the provider; who uses it. While the key's secret is missing, Edit
//! connection leads the head's actions.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{
  connection::{AgentConnection, AgentProvider, ApiProvider, ConnectionStatus},
  secret::Secret,
};

use super::{
  Actions, Inspecting, Parts,
  agent::{checked_detail, menu, models, program_rows},
  check_button, rename::title_with_rename, result, sentence, sentence_tone, shown,
};
use crate::{
  screens::{
    connections::words::{agent_glyph, connection_status},
    project_settings::kit::{Group, SettingRow, Tone, value},
  },
  theme::ExtraTypography,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

pub(super) fn parts(
  ctx: &mut Ctx,
  (connection, provider): (&AgentConnection, &ApiProvider),
  used: Element,
  inspecting: &Inspecting<'_>,
  actions: &Actions,
) -> Parts {
  let secrets = &inspecting.listed.secrets;
  let key = secrets.iter().find(|secret| secret.id() == provider.key());
  let status = connection.status_with(secrets);
  let shown_status = shown(inspecting, connection_status(connection, status));
  let mut runs_on = Group::new("connection-provider", ctx.t("connections.runsOn")).rows(program_rows(ctx, connection, inspecting));
  runs_on = runs_on.rows(runs_on_rows(ctx, connection.provider(), provider, key));
  let check = check_button(inspecting, ("connection-test", ctx.t("connections.checkAgain")), actions);
  let head_actions = if status == ConnectionStatus::KeyMissing {
    let edit = ActionButton::new("connection-edit-key", ButtonKind::Primary, ctx.t("connections.edit"))
      .icon(Icon::Pencil)
      .opens((actions.edit)(connection.id()));
    vec![edit.into(), check.into()]
  } else {
    vec![check.into()]
  };
  let said = match (status, key) {
    (ConnectionStatus::KeyMissing, _) => Some(ctx.t("connections.apiProvider.detailKeyMissing")),
    (ConnectionStatus::Connected | ConnectionStatus::NotChecked, _) => None,
    (ConnectionStatus::ExecutableMissing, _) => Some(ctx.t("connections.detail.executableMissing")),
    (ConnectionStatus::NotRecognized, _) => Some(ctx.t("connections.detail.notRecognized")),
    _ => Some(ctx.t("connections.detail.couldNotRun")),
  };
  let mut notices: Vec<Element> = said
    .map(|said| sentence("connection-inspector-sentence", &said, sentence_tone(shown_status)))
    .into_iter()
    .collect();
  notices.extend(result::agent(ctx, connection, inspecting));
  Parts {
    glyph: agent_glyph(connection.provider()),
    title: title_with_rename(ctx, (inspecting, actions), (connection.id(), connection.name().as_str())),
    status: shown_status,
    status_detail: checked_detail(ctx, connection, inspecting),
    actions: head_actions,
    menu: menu(ctx, connection, (inspecting, actions)),
    notices,
    groups: vec![runs_on.into(), models(ctx, connection, inspecting, actions), used],
    question: None,
  }
}

/// *Runs on* an API provider: the API its CLI speaks, the base URL with where requests go, and the key's secret by
/// name.
fn runs_on_rows(ctx: &mut Ctx, cli: AgentProvider, provider: &ApiProvider, key: Option<&Secret>) -> Vec<Element> {
  let interface = match cli {
    AgentProvider::CodexCli => "connections.apiProvider.wireCodex",
    _ => "connections.apiProvider.wireClaude",
  };
  let (key_words, key_tone) = match key {
    Some(secret) => (secret.name().as_str().into(), Tone::Muted),
    None => (ctx.t("connections.apiProvider.keyGone"), Tone::Danger),
  };
  let mut rows = vec![
    SettingRow::new("connection-runs-on", ctx.t("connections.runsOn"))
      .hint(ctx.t(interface))
      .control(value("connection-runs-on-value", &ctx.t("connections.apiProvider.label")))
      .render(ctx),
    SettingRow::new("connection-provider-base-url", ctx.t("connections.apiProvider.url"))
      .hint(ctx.t_args("connections.apiProvider.sentTo", [("host", provider.base_url().host())]))
      .control(
        value("connection-provider-base-url-value", provider.base_url().as_str())
          .variant(ExtraTypography::PropertyMono.role()),
      )
      .render(ctx),
  ];
  let key_row = SettingRow::new("connection-provider-key", ctx.t("connections.apiProvider.key"))
    .toned(ctx.t("connections.apiProvider.keyHint"), Tone::Muted)
    .control(
      value("connection-provider-key-value", &key_words)
        .variant(ExtraTypography::PropertyMono.role())
        .color(key_tone.color()),
    );
  rows.push(key_row.render(ctx));
  rows
}
