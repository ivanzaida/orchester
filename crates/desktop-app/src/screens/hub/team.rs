//! The team's configuration in a shared project's pipelines (`Shared configuration · redesign` `UC5xk`,
//! `docs/design/shared-configuration.md`): the line saying who may change it (01b `P8tXS`, 03 `pjEjf`), and who the
//! admins are, which the Agents hub says too. A team's pipeline is read from the team server and changed there
//! (`docs/design/team-configuration-live.md`), so for an admin the line says so. A local project shows none of it.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::dimension::Dimension,
};
use orchester_core::time::Timestamp;
use orchester_workspace::{display, i18n::NAMESPACE, sharing::team::TeamConfiguration};

use crate::{
  project::ShownProject,
  theme::{HubText, IconSize},
  ui::icon::{Icon, icon},
};

/// The team's configuration of `project` while it is shared; `None` while it is local or not read yet. A screen that
/// must not take the unread for local reads [`TeamRead`] itself.
pub(crate) fn of(project: &ShownProject) -> Option<TeamConfiguration> {
  project.feed.team.get().configuration()
}

/// Whether the person may change `project`'s resources: always in a local project, an admin's in a shared one.
pub(crate) fn may_change(team: Option<&TeamConfiguration>) -> bool {
  team.is_none_or(|team| team.may_change)
}

/// The line over a team's pipelines (`Shared line` `t3ABe9` in 01, `Read-only line` `z8Triu` in 03): a member's says
/// who may change them, an admin's that what they save goes to the team server at once (D-043).
pub(crate) fn line(ctx: &Ctx, id: &str, team: &TeamConfiguration) -> Row {
  if team.may_change {
    let text = match &team.server {
      Some((name, _)) => ctx.t_args("sharing.config.pipelinesSaved", [("team", name.to_string())]),
      None => ctx.t("sharing.config.pipelinesSavedNoServer"),
    };
    note_line(id, (Icon::Users, &text))
  } else {
    note_line(
      id,
      (
        Icon::Lock,
        &ctx.t_args("sharing.config.readOnly", [("admins", admins(ctx, team))]),
      ),
    )
  }
  .describe("may-change", team.may_change.to_string())
}

/// `text` after `glyph`, as the line over a library says it.
fn note_line(id: &str, (glyph, text): (Icon, &str)) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(text)
        .id(format!("{id}-text"))
        .variant(HubText::TeamLine.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// `dana-k (owner) and olha (server admin)`: the owners and the server's admins as last read. A server that does not
/// name its admins leaves them by their role: `dana-k (owner) and the server's admins`.
pub(crate) fn admins(ctx: &Ctx, team: &TeamConfiguration) -> String {
  let named = |key: &str, names: &[Arc<str>]| {
    names
      .iter()
      .map(|name| ctx.t_args(key, [("name", name.to_string())]).to_string())
      .collect::<Vec<_>>()
  };
  let owners = named("sharing.config.owner", &team.owners);
  if team.admins.is_empty() {
    if owners.is_empty() {
      return ctx.t("sharing.config.noOwners").to_string();
    }
    let owners = owners.join(&ctx.t("sharing.config.separator"));
    return ctx
      .t_args("sharing.config.ownersAndAdmins", [("owners", owners)])
      .to_string();
  }
  let mut all = owners;
  all.extend(named("sharing.config.serverAdmin", &team.admins));
  match all.split_last() {
    Some((last, rest)) if !rest.is_empty() => ctx
      .t_args(
        "sharing.config.and",
        [
          ("first", rest.join(&ctx.t("sharing.config.separator"))),
          ("last", last.clone()),
        ],
      )
      .to_string(),
    _ => all.concat(),
  }
}

/// When a version was published, as a row says it: `today 10:12`, or its day, `Oct 27`.
pub(crate) fn when(ctx: &Ctx, at: Timestamp) -> String {
  if at.local_day_start() == Timestamp::now().local_day_start() {
    return ctx
      .t_args("sharing.config.today", [("time", display::clock(at))])
      .to_string();
  }
  let (month, day) = display::month_day(at);
  let month = ctx.t_ns(NAMESPACE, display::MONTH_KEYS[month]).to_string();
  ctx
    .t_args("sharing.config.day", [("month", month), ("day", day.to_string())])
    .to_string()
}
