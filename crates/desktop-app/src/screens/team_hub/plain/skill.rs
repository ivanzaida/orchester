//! What a skill's panel shows besides its head (`From the team` in `Agents · Skills` `c9YnL`): its description, its
//! files, where it is added when that is not every run, and who uses it. The design's *Loads on* row (the CLIs that
//! load it) is not here: a skill records no such thing.

use lurq::{components::Row, node::Element};
use orchester_workspace::agents::team_text::{Added, TeamSkillSheet};

use super::{added_row, from_team, used_by_row};
use crate::screens::team_hub::rows;

pub(super) fn sections(ctx: &lurq::app::ctx::Ctx, sheet: &TeamSkillSheet) -> Vec<Element> {
  let mut rows = Vec::new();
  if !sheet.description.is_empty() {
    rows.push(rows::row(
      "team-hub-from-team-description",
      &ctx.t("team_hub.plain.description"),
      None,
      rows::words("team-hub-from-team-description-value", &sheet.description),
    ));
  }
  rows.push(files_row(ctx, &sheet.files));
  if !matches!(sheet.added, Added::Project) {
    rows.push(added_row(ctx, &sheet.added));
  }
  rows.push(used_by_row(ctx, &sheet.used_by));
  vec![from_team(ctx, rows).into()]
}

/// *Files*: `SKILL.md` and its reference files, each in a tag.
fn files_row(ctx: &lurq::app::ctx::Ctx, files: &[String]) -> Row {
  rows::row(
    "team-hub-from-team-files",
    &ctx.t("team_hub.plain.files"),
    None,
    rows::tags("team-hub-from-team-files-value", files.iter().cloned()),
  )
}
