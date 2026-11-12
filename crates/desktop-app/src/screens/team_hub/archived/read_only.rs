//! What the team holds of an archived resource, read-only until it is restored (`From the team · read-only until it is
//! restored` in A36 `qvMZD`): the *From the team* of its kind's panel, as values. A latest version this computer cannot
//! read shows nothing of it.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_workspace::agents::team_archived::{ArchivedSheet, TeamArchived};

use crate::screens::team_hub::{mcp, plain, profile, rows::FromTeam, tool};

pub(super) fn sections(ctx: &Ctx, archived: &TeamArchived) -> Vec<Element> {
  let note = FromTeam::Archived;
  match &archived.sheet {
    Some(ArchivedSheet::Profile(sheet)) => vec![profile::from_team::section(ctx, sheet, note).into()],
    Some(ArchivedSheet::McpServer(sheet)) => vec![mcp::from_team::section(ctx, sheet, note).into()],
    Some(ArchivedSheet::Tool(sheet)) => vec![tool::from_team::section(ctx, sheet, note).into()],
    Some(ArchivedSheet::Instruction(sheet)) => plain::instruction::sections(ctx, sheet, note),
    Some(ArchivedSheet::Skill(sheet)) => plain::skill::sections(ctx, sheet, note),
    None => Vec::new(),
  }
}
