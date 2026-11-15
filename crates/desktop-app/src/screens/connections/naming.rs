//! The connection's name on Add connection: the one the page suggests for the CLI and project, and why a name
//! cannot be the connection's.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::name::Name;
use orchester_workspace::messages;

use crate::ui::message::message_text;

/// Why `name` cannot be a connection's name while the project's other agent and GitHub connections have the names
/// `taken`: the rule of adding one, and of renaming one.
pub(in crate::screens::connections) fn problem_among<'a>(
  ctx: &Ctx,
  mut taken: impl Iterator<Item = &'a str>,
  name: &str,
) -> Option<Arc<str>> {
  if let Err(error) = Name::new(name) {
    return Some(message_text(ctx, &messages::name_error(&error)));
  }
  taken
    .any(|taken| taken == name.trim())
    .then(|| ctx.t("connections.dialog.nameTaken"))
}
