//! The foot of the stopped card in a team project (`Restore` in `Run state · Stopped · archived` of A22 `pUu78`):
//! restoring a team resource is the admins' (`docs/design/team-configuration.md`, rule 7), and this app has no restore
//! of its own, so a member's card says who the admins are, and an admin's says where it is restored.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::TaskContext;
use crate::screens::hub::team;

/// The foot of the card of a team task whose run an archive stopped; none in a local project or while the team's
/// configuration is not read.
pub(super) fn foot(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  task.team.as_ref()?;
  let team = team::of(&task.project)?;
  let hint = if team.may_change {
    ctx.t("summary.archived.restoreOnServer")
  } else {
    let admins = team::admins(ctx, &team);
    ctx.t_args("summary.archived.adminsOnly", [("admins", admins)])
  };
  Some(
    Row::new()
      .id("task-archived-foot")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(Row::new().flex(1.0))
      .child(
        Text::new(&hint)
          .id("task-archived-foot-hint")
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      )
      .into(),
  )
}
