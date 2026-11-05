//! The lines under a team project's Tasks header (`Shared line` `kY3Fg` and `Read-only line` `dLyl8` in 01b `zv7b6`;
//! `zUEa1` in 01 `QdL7N`): where the project lives, that its tasks are read from the server and the person's role as
//! the server says it (12, a 14 cloud, `·` between, *Sharing settings* at the end); and for a member, one calm line
//! naming who changes the team's tasks (`crate::team::lock`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Spacer, Text},
  layout::Alignment,
  node::dimension::Dimension,
};
use orchester_api::v2::CallerRole;
use orchester_core::id::ProjectId;
use orchester_workspace::sharing::project::TeamAdmins;

use crate::{
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route},
  screens::settings::orchester::shown_address,
  team::lock::{lock_words, locked},
  theme::IconSize,
  ui::{
    icon::{Icon, icon},
    pressable::opening,
  },
};

/// *Team project on orchester.acme.dev/kontur · Read from the server · You're a member*, and *Sharing settings*.
pub(in crate::screens::tasks) fn team_line(
  ctx: &mut Ctx,
  project: ProjectId,
  (place, role): (&str, CallerRole),
) -> Row {
  let navigation = Navigation::of(ctx);
  let quiet = |words: &str| {
    Text::new(words)
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextMuted)
      .nowrap()
  };
  let dot = || quiet(&ctx.t("tasks.shared.separator"));
  let role = ctx.t(match role {
    CallerRole::Member => "tasks.team.role.member",
    CallerRole::Admin => "tasks.team.role.admin",
    CallerRole::ServerAdmin => "tasks.team.role.serverAdmin",
  });
  let settings = opening(navigation.to(Route::Project(
    project,
    ProjectPage::Settings(ProjectSettingsTab::Sharing),
  )))
  .id("tasks-team-settings")
  .align_items(Alignment::Center)
  .child(
    Text::new(&ctx.t("tasks.shared.settings"))
      .variant(TypographyStyle::Description)
      .nowrap(),
  );
  Row::new()
    .id("tasks-team-line")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Cloud, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t_args("tasks.team.on", [("place", shown_address(place))]))
        .id("tasks-team-place")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .child(dot())
    .child(quiet(&ctx.t("tasks.team.fromServer")).id("tasks-team-from-server"))
    .child(dot())
    .child(quiet(&role).id("tasks-team-role"))
    .child(Spacer::new().flex(1.0))
    .child(settings)
}

/// *Only Admins change team tasks, modules and tags: dana-k (Admin) and olha (Server admin). You read, run and claim.*;
/// on the board, that they move cards too. Without the admins' names until the members are read.
pub(in crate::screens::tasks) fn lock_line(ctx: &Ctx, admins: Option<&TeamAdmins>, board: bool) -> Row {
  let keys = if board {
    ("tasks.team.readOnlyBoard", "tasks.team.readOnlyBoardUnnamed")
  } else {
    ("tasks.team.readOnly", "tasks.team.readOnlyUnnamed")
  };
  locked("tasks-team-read-only", &lock_words(ctx, admins, keys))
}
