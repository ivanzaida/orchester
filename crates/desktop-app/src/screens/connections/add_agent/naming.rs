//! The connection's name on Add connection: the one the page suggests for the CLI and project, and why a name
//! cannot be the connection's.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, i18n::I18n},
  core::Signal,
};
use orchester_core::{connection::AgentProvider, name::Name};
use orchester_workspace::{connections::data::ProjectConnections, messages};

use crate::ui::message::message_text;

/// Puts `new` in the name while it holds the last suggestion or nothing: a name the person typed stays.
pub(super) fn suggest(name: &Signal<String>, suggested: &Signal<String>, new: String) {
  let current = name.get_untracked();
  let previous = suggested.get_untracked();
  if (current.is_empty() || current == previous) && current != new {
    name.set(new.clone());
  }
  if previous != new {
    suggested.set(new);
  }
}

/// `{CLI} · {project}`, numbered when another connection of the project has the name.
pub(super) fn suggested_name(
  i18n: &I18n,
  listed: &ProjectConnections,
  provider: AgentProvider,
  project: &str,
) -> String {
  let short = i18n.t(short_key(provider));
  let taken = |name: &str| taken_names(listed).any(|taken| taken == name);
  let plain = i18n.t_args(
    "connections.dialog.suggestedName",
    [("provider", &*short), ("project", project)],
  );
  if !taken(&plain) {
    return plain.to_string();
  }
  (2_usize..)
    .map(|number| {
      i18n
        .t_args(
          "connections.dialog.suggestedNameNumbered",
          [
            ("provider", &*short),
            ("project", project),
            ("number", &number.to_string()),
          ],
        )
        .to_string()
    })
    .find(|name| !taken(name))
    .expect("only finitely many connection names are taken")
}

const fn short_key(provider: AgentProvider) -> &'static str {
  match provider {
    AgentProvider::CodexCli => "connections.provider.codexShort",
    AgentProvider::ClaudeCodeCli => "connections.provider.claudeShort",
    AgentProvider::AnthropicApi => "connections.provider.anthropicApiShort",
    AgentProvider::OpenAiApi => "connections.provider.openaiApiShort",
  }
}

/// The names the project's agent and GitHub connections have; they share one list.
fn taken_names(listed: &ProjectConnections) -> impl Iterator<Item = &str> {
  let agents = listed.connections.iter().map(|connection| connection.name().as_str());
  agents.chain(listed.github.iter().map(|connection| connection.name().as_str()))
}

/// Why `name` cannot be the connection's name.
pub(super) fn name_problem(ctx: &Ctx, listed: &ProjectConnections, name: &str) -> Option<Arc<str>> {
  if let Err(error) = Name::new(name) {
    return Some(message_text(ctx, &messages::name_error(&error)));
  }
  taken_names(listed)
    .any(|taken| taken == name.trim())
    .then(|| ctx.t("connections.dialog.nameTaken"))
}
