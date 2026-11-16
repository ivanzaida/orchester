//! *Role* of a team profile the person does not edit (M1 `oPF2o`, A26 `WSw0W` as a member reads it): its role, what it
//! answers for and the role instructions as values, as the rest of *From the team* is read, *None* where it says
//! nothing.

use lurq::{app::ctx::Ctx, layout::Alignment, node::Element};
use orchester_workspace::agents::{role::RoleText, team_profile::TeamProfileSheet};

use crate::screens::team_hub::rows::{self, Sub};

/// The section; none before the profile's version is read.
pub(super) fn section(ctx: &Ctx, sheet: &TeamProfileSheet) -> Option<Element> {
  let latest = sheet.edit.as_ref()?;
  let role = RoleText::of_team(&latest.body);
  let none = ctx.t("team_hub.profile.none");
  let or_none = |text: &str| {
    if text.trim().is_empty() {
      none.to_string()
    } else {
      text.to_owned()
    }
  };
  let id = "team-hub-role";
  let rows = vec![
    rows::row(
      &format!("{id}-name"),
      &ctx.t("hub.role.name"),
      None,
      rows::words(&format!("{id}-name-value"), &role.role),
    ),
    rows::row(
      &format!("{id}-responsibilities"),
      &ctx.t("hub.role.responsibilities"),
      Some(Sub::quiet(ctx.t("hub.role.responsibilitiesSub"))),
      rows::words(
        &format!("{id}-responsibilities-value"),
        &or_none(&role.responsibilities),
      ),
    )
    .align_items(Alignment::Start),
    rows::row(
      &format!("{id}-instructions"),
      &ctx.t("hub.role.instructions"),
      Some(Sub::quiet(ctx.t("hub.role.instructionsSub"))),
      rows::words(&format!("{id}-instructions-value"), &or_none(&role.instructions)),
    )
    .align_items(Alignment::Start),
  ];
  Some(rows::section(id, &ctx.t("hub.role.title"), Vec::new(), rows).into())
}
