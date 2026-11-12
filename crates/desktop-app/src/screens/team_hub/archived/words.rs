//! What an archived resource's panel says (`Notice` in A36 `qvMZD`): what archiving did and what *Restore* does, and
//! why a restore was refused.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::resource::ResourceKind;
use orchester_workspace::{
  agents::team_archived::TeamArchived, readiness::archive_blocks_profiles, sharing::project::NotRestored,
};

use crate::{
  screens::team_hub::archive::words::listed,
  theme::HubText,
  ui::{
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

/// *Archived: hidden from lists and pickers*: the runs archiving stopped, which stay stopped, what Restore puts back,
/// and the profiles that stay not ready until then.
pub(super) fn notice(ctx: &Ctx, archived: &TeamArchived) -> Element {
  let mut words = stopped(ctx, archived).to_string();
  if let Some(blocks) = blocks(ctx, archived) {
    words = ctx
      .t_args(
        "team_hub.archived.sentences",
        [("first", words.as_str()), ("then", &*blocks)],
      )
      .to_string();
  }
  Notice::new(
    "team-hub-archived-notice",
    NoticeTone::Neutral,
    ctx.t("team_hub.archived.title"),
  )
  .icon(Icon::Archive)
  .title_style(HubText::NoticeTitle.role())
  .body(words)
  .flat()
  .into()
}

/// The runs its archive stopped here and what Restore puts back.
fn stopped(ctx: &Ctx, archived: &TeamArchived) -> Arc<str> {
  let version = archived.number.map_or_else(
    || ctx.t("team_hub.archived.noVersion").to_string(),
    |number| number.to_string(),
  );
  let name = archived.name.as_str();
  if archived.stopped.is_empty() {
    return ctx.t_args(
      "team_hub.archived.stoppedNone",
      [("name", name), ("version", version.as_str())],
    );
  }
  let runs: Vec<String> = archived
    .stopped
    .iter()
    .map(|run| {
      ctx
        .t_args(
          "team_hub.archived.run",
          [("key", run.key.as_str()), ("title", run.title.as_str())],
        )
        .to_string()
    })
    .collect();
  let named: Vec<&str> = runs.iter().map(String::as_str).collect();
  ctx.t_args(
    if archived.stopped.len() == 1 {
      "team_hub.archived.stoppedOne"
    } else {
      "team_hub.archived.stopped"
    },
    [
      ("count", archived.stopped.len().to_string()),
      ("runs", listed(ctx, &named).to_string()),
      ("name", name.to_owned()),
      ("version", version),
    ],
  )
}

/// *Until then Builder, which chooses it, is not ready on any computer.*: the profiles that still name a resource whose
/// archive keeps them from running; none for a kind that does not, or when no profile names it.
fn blocks(ctx: &Ctx, archived: &TeamArchived) -> Option<Arc<str>> {
  if !archive_blocks_profiles(archived.kind) {
    return None;
  }
  let names = archived.sheet.as_ref()?.used_by();
  let key = match (archived.kind == ResourceKind::McpServer, names) {
    (_, []) => return None,
    (true, [_]) => "team_hub.archived.blocksMcpOne",
    (true, _) => "team_hub.archived.blocksMcp",
    (false, [_]) => "team_hub.archived.blocksOne",
    (false, _) => "team_hub.archived.blocks",
  };
  let names: Vec<&str> = names.iter().map(String::as_str).collect();
  Some(ctx.t_args(key, [("names", listed(ctx, &names))]))
}

/// Why the team server did not restore `name`.
pub(super) fn refusal(ctx: &Ctx, name: &str, refused: &NotRestored) -> Arc<str> {
  match refused {
    NotRestored::Refused { code } => match code.as_str() {
      "not_admin" => ctx.t("team_hub.archived.refused.notAdmin"),
      "revision_conflict" => ctx.t_args("team_hub.archived.refused.conflict", [("name", name)]),
      other => ctx.t_args("team_hub.archived.refused.other", [("code", other)]),
    },
    NotRestored::Failed(message) => message_text(ctx, message),
  }
}
