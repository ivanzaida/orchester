//! The inspector of an agent CLI connection on an API provider (`docs/design/api-providers.md`; `y2qhJ`, `tnEKp`,
//! `b3aoUZ`): its properties with *API key* as the authentication and plan limits not read; *Runs on* with the API its
//! CLI speaks, the base URL, the key by its secret's name (never its value) and where requests go; the models typed for
//! the provider; who uses it; and in the footer what its status means, with Test and Edit connection (Edit first while
//! the key's secret is missing).

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  layout::Alignment,
  node::Element,
};
use orchester_core::{
  connection::{AgentConnection, AgentProvider, ApiProvider, ConnectionStatus},
  secret::Secret,
};

use super::{
  Actions, Inspecting, Parts,
  agent::{models, properties},
  check_button, footer, footer_action, result,
};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
  inspector::{self, PropertyValue, property},
};

pub(super) fn parts(
  ctx: &mut Ctx,
  (connection, provider): (&AgentConnection, &ApiProvider),
  (header, used): (Element, Element),
  inspecting: &Inspecting<'_>,
  actions: &Actions,
) -> Parts {
  let secrets = &inspecting.listed.secrets;
  let key = secrets.iter().find(|secret| secret.id() == provider.key());
  let body = vec![
    properties(ctx, connection, inspecting).into(),
    runs_on(ctx, connection.provider(), provider, key),
    models(ctx, connection, inspecting, actions),
    used,
  ];
  let status = connection.status_with(secrets);
  let missing = status == ConnectionStatus::KeyMissing;
  let (edit, id) = (actions.edit.clone(), connection.id());
  let kind = if missing {
    ButtonKind::Primary
  } else {
    ButtonKind::Secondary
  };
  let edit = footer_action(
    inspecting,
    ActionButton::new("connection-edit", kind, ctx.t("connections.edit")),
    Icon::Pencil,
  )
  .opens(edit(id));
  let test = footer_action(
    inspecting,
    check_button(inspecting, ("connection-test", ctx.t("connections.test")), actions),
    Icon::PlugZap,
  );
  let buttons = if missing {
    vec![edit.into(), test.into()]
  } else {
    vec![test.into(), edit.into()]
  };
  let sentence = match (status, key) {
    (ConnectionStatus::KeyMissing, _) => ctx.t("connections.apiProvider.detailKeyMissing"),
    (ConnectionStatus::Connected, Some(key)) => ctx.t_args(
      "connections.apiProvider.detailConnected",
      [("secret", key.name().as_str())],
    ),
    (ConnectionStatus::NotChecked, _) => ctx.t("connections.detail.notChecked"),
    (ConnectionStatus::ExecutableMissing, _) => ctx.t("connections.detail.executableMissing"),
    (ConnectionStatus::NotRecognized, _) => ctx.t("connections.detail.notRecognized"),
    _ => ctx.t("connections.detail.couldNotRun"),
  };
  let tested = result::agent(ctx, connection, inspecting).into_iter().collect();
  Parts {
    header,
    body,
    footer: Some(footer(
      ctx,
      inspecting,
      (&sentence, tested),
      (vec![buttons], Vec::new()),
    )),
  }
}

/// *Runs on*: the API the CLI speaks, the base URL, the key's secret by name, and where requests go.
fn runs_on(ctx: &Ctx, cli: AgentProvider, provider: &ApiProvider, key: Option<&Secret>) -> Element {
  let interface = match cli {
    AgentProvider::CodexCli => "connections.apiProvider.wireCodex",
    _ => "connections.apiProvider.wireClaude",
  };
  let (key, kind) = match key {
    Some(secret) => (secret.name().as_str().into(), PropertyValue::Mono),
    None => (ctx.t("connections.apiProvider.keyGone"), PropertyValue::Danger),
  };
  inspector::section("connection-provider", SpacingSize::Sm)
    .child(inspector::section_title(
      "connection-provider-title",
      &ctx.t("connections.apiProvider.label"),
      None,
    ))
    .child(property(
      "connection-provider-interface",
      &ctx.t("connections.apiProvider.interface"),
      &ctx.t(interface),
      PropertyValue::Plain,
    ))
    // A base URL that wraps keeps its label across from it, the row as compact as the others (`y2qhJ`).
    .child(
      property(
        "connection-provider-base-url",
        &ctx.t("connections.apiProvider.url"),
        provider.base_url().as_str(),
        PropertyValue::Mono,
      )
      .align_items(Alignment::Center),
    )
    .child(property(
      "connection-provider-key",
      &ctx.t("connections.apiProvider.key"),
      &key,
      kind,
    ))
    .child(inspector::note(
      "connection-provider-sent-to",
      Icon::Globe,
      &ctx.t_args("connections.apiProvider.sentTo", [("host", provider.base_url().host())]),
    ))
    .into()
}
