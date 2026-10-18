//! The inspector rows of an agent CLI connection on an API provider (`docs/design/api-providers.md`; no Pencil frame
//! yet): the API its CLI speaks, the base URL, the key secret by name (never its value), and where the agents'
//! requests go. Such a connection has no account and no plan limits; its models are the provider's, typed.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  node::Element,
};
use orchester_core::{
  connection::{AgentProvider, ApiProvider},
  secret::Secret,
};

use crate::ui::{
  icon::Icon,
  inspector::{self, PropertyValue, property},
};

/// The provider's rows and the note on where requests go.
pub(super) fn rows(ctx: &Ctx, cli: AgentProvider, provider: &ApiProvider, secrets: &[Secret]) -> Element {
  let interface = match cli {
    AgentProvider::CodexCli => "connections.apiProvider.wireCodex",
    _ => "connections.apiProvider.wireClaude",
  };
  let key = secrets
    .iter()
    .find(|secret| secret.id() == provider.key() && !secret.is_archived());
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
    .child(property(
      "connection-provider-base-url",
      &ctx.t("connections.apiProvider.url"),
      provider.base_url().as_str(),
      PropertyValue::Mono,
    ))
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
