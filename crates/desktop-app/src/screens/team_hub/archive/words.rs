//! What the confirmation's lists say: a row's kind and the names a note lists.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_api::SharedResourceKind;

/// The key of what `kind` is called in a row (*agent profile*).
pub(super) const fn kind_key(kind: SharedResourceKind) -> &'static str {
  match kind {
    SharedResourceKind::AgentProfile => "team_archive.kind.profile",
    SharedResourceKind::Pipeline => "team_archive.kind.pipeline",
    SharedResourceKind::Instruction => "team_archive.kind.instruction",
    SharedResourceKind::Skill => "team_archive.kind.skill",
    SharedResourceKind::Tool => "team_archive.kind.tool",
    SharedResourceKind::McpServer => "team_archive.kind.mcp",
    SharedResourceKind::TeamReviewer => "team_archive.kind.reviewer",
  }
}

/// `names` as a person says them: *Builder*, *Builder and Designer*, *Builder, Designer and Reviewer*.
pub(in crate::screens) fn listed(ctx: &Ctx, names: &[&str]) -> Arc<str> {
  match names {
    [] => Arc::from(""),
    [only] => Arc::from(*only),
    [first, second] => ctx.t_args("team_archive.names.two", [("first", *first), ("second", *second)]),
    [rest @ .., last] => ctx.t_args(
      "team_archive.names.more",
      [("rest", rest.join(", ").as_str()), ("last", *last)],
    ),
  }
}
