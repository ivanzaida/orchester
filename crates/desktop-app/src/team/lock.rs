//! The calm line a member reads on a team project's screens (`Read-only line` `dLyl8` in 01b `zv7b6`; `HAhq7` in 01c,
//! `zlXIL` in 01d, `ruWE4` in 01e): a 13 lock, then who changes what in 12.5 `text_secondary`, 8 apart, 2 over and
//! under; and the Admins' names it holds, as the members read named them.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{dimension::Dimension, padding::Padding},
};
use orchester_workspace::sharing::project::TeamAdmins;

use crate::{
  theme::{ExtraSpacing, HubText, IconSize},
  ui::icon::{Icon, icon},
};

/// The line saying `words`, its element `id`, its words `<id>-text`.
pub fn locked(id: &str, words: &str) -> Row {
  flat(id, words).padding(Padding::vertical(ExtraSpacing::Tight.role()))
}

/// [`locked`] without the 2 over and under it, for a page whose design spaces the line itself (the Agents hub's
/// `Read-only line` `O4b6B` in M2 `dijUt`).
pub fn flat(id: &str, words: &str) -> Row {
  marked(id, Icon::Lock, words)
}

/// [`locked`] with a `shield-check` in place of the lock: a line that says what Admins are, not what a member cannot do
/// (Tasks' Admin line, T8 `vdSWS`).
pub fn shielded(id: &str, words: &str) -> Row {
  marked(id, Icon::ShieldCheck, words).padding(Padding::vertical(ExtraSpacing::Tight.role()))
}

fn marked(id: &str, glyph: Icon, words: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(words)
        .id(format!("{id}-text"))
        .variant(HubText::TeamLine.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// `dana-k (Admin) and olha (Server admin)`; empty when nobody is named.
pub fn admin_names(ctx: &Ctx, admins: &TeamAdmins) -> String {
  let with_roles = admins
    .admins
    .iter()
    .map(|name| ctx.t_args("tasks.team.admin", [("name", name.as_ref())]).to_string())
    .chain(admins.server_admins.iter().map(|name| {
      ctx
        .t_args("tasks.team.serverAdmin", [("name", name.as_ref())])
        .to_string()
    }));
  listed(ctx, &with_roles.collect::<Vec<_>>())
}

/// `dana-k and olha`: the names without their roles, where a line names who can close the task (`g9S4k`, `tlw0r`);
/// empty when nobody is named.
pub fn plain_names(ctx: &Ctx, admins: &TeamAdmins) -> String {
  let names: Vec<String> = admins
    .admins
    .iter()
    .chain(&admins.server_admins)
    .map(ToString::to_string)
    .collect();
  listed(ctx, &names)
}

/// `all` as a sentence's list: `a, b and c`.
fn listed(ctx: &Ctx, all: &[String]) -> String {
  match all.split_last() {
    Some((last, rest)) if !rest.is_empty() => ctx
      .t_args(
        "tasks.team.and",
        [
          ("first", rest.join(&ctx.t("tasks.team.separator"))),
          ("last", last.clone()),
        ],
      )
      .to_string(),
    _ => all.concat(),
  }
}

/// The words of a lock line: `named` with the Admins' names once they are read and name someone, else `unnamed`.
pub fn lock_words(ctx: &Ctx, admins: Option<&TeamAdmins>, (named, unnamed): (&str, &str)) -> String {
  match admins
    .map(|admins| admin_names(ctx, admins))
    .filter(|names| !names.is_empty())
  {
    Some(names) => ctx.t_args(named, [("admins", names)]).to_string(),
    None => ctx.t(unnamed).to_string(),
  }
}
