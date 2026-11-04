//! A team project's Tasks list while its team data cannot be read (`06 · Tasks · team data unavailable` `GR3RS`): the
//! state's card under the header (`State · team data unavailable` `l6EMj`: a 36 tile with the 18 `cloud-off`, the title
//! in 15/600, why in 12.5 `text_secondary`, *Try again*), 40 under the header; then, 24 under it, what runs on this
//! computer (`On this computer` `XTBXg`: its heading and count, a 40 high row per run, the line saying runs keep
//! going), read from this computer's runs, never from the team's tasks. The same card says why when this computer is
//! signed out of the server, the person is no longer a member, or the server refused the read.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::run::TaskPhase;
use orchester_workspace::tasks::{data::ProjectTasks, list::Latest};

use super::super::shared::Shared;
use crate::{
  screens::settings::orchester::moment,
  team::TeamFailure,
  theme::{ExtraSpacing, ExtraTypography, HubText, IconSize, TasksSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    state::EmptyState,
  },
};

/// A run's row (`KON-12` `FNPd2`).
const ROW_HEIGHT: f32 = 40.0;

/// The card and what runs on this computer; `retry` reads the team's data again.
pub(in crate::screens::tasks) fn outage(
  ctx: &mut Ctx,
  (failure, shared, project): (&TeamFailure, Option<&Shared>, &str),
  local: &ProjectTasks,
  retry: impl Fn() + Send + Sync + 'static,
) -> Column {
  let server = shared.map_or("", |shared| shared.sharing.server_name());
  let (title, body) = match failure {
    TeamFailure::Unavailable { since } => (
      ctx.t("tasks.team.unavailable.title"),
      ctx.t_args(
        "tasks.team.unavailable.body",
        [("server", server.to_owned()), ("time", moment(ctx, *since))],
      ),
    ),
    TeamFailure::SignedOut => (
      ctx.t("tasks.team.unavailable.title"),
      ctx.t_args("tasks.team.signedOut", [("server", server), ("project", project)]),
    ),
    TeamFailure::NotMember => (
      ctx.t("tasks.team.unavailable.title"),
      ctx.t_args("tasks.team.notMember", [("server", server), ("project", project)]),
    ),
    TeamFailure::Refused => (
      ctx.t("tasks.team.refused.title"),
      ctx.t_args("tasks.team.refused.body", [("server", server)]),
    ),
  };
  let again = ActionButton::new("tasks-team-try-again", ButtonKind::Primary, ctx.t("common.tryAgain"))
    .icon(Icon::RefreshCw)
    .on_click(retry);
  let card = EmptyState::new("tasks-team-unavailable", Icon::CloudOff, title, body)
    .wide()
    .action(again);
  Column::new()
    .id("tasks-team-outage")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(TasksSpacing::OutageTop.role()))
    .spacing(SpacingSize::Lg)
    .describe("state", "unavailable")
    .child(card)
    .with_children(running(ctx, local))
}

/// *Running on this computer* with each run at work here and the line saying runs keep going; `None` without one.
fn running(ctx: &Ctx, local: &ProjectTasks) -> Option<Column> {
  let rows: Vec<Element> = local
    .tasks
    .iter()
    .filter_map(|task| {
      let Some(Latest::Run(latest)) = local.phases.get(&task.id()) else {
        return None;
      };
      matches!(latest.phase, TaskPhase::Running | TaskPhase::Waiting).then(|| {
        let key = task.key().to_string();
        let step = latest.step.as_ref().map_or_else(
          || ctx.t_args("tasks.team.run", [("number", latest.number.to_string())]),
          |step| {
            ctx.t_args(
              "tasks.team.runStep",
              [
                ("number", format!("{:02}", step.number)),
                ("step", step.name.to_string()),
                ("run", latest.number.to_string()),
              ],
            )
          },
        );
        row(&key, task.details().title.as_str(), &step)
      })
    })
    .collect();
  if rows.is_empty() {
    return None;
  }
  let heading = Row::new()
    .id("tasks-team-running")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("tasks.team.running")).variant(ExtraTypography::Strong.role()))
    .child(
      Text::new(&rows.len().to_string())
        .id("tasks-team-running-count")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    );
  let note = Row::new()
    .id("tasks-team-outage-note")
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Tight.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Info, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("tasks.team.outageNote"))
        .variant(HubText::TeamLine.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    );
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(heading)
      .with_children(rows)
      .child(note),
  )
}

/// One run at work here: its loader, key, title and where the run is.
fn row(key: &str, title: &str, step: &str) -> Element {
  Row::new()
    .id(format!("tasks-team-running-{key}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::LoaderCircle, IconSize::Md, PaletteColor::TextSecondary))
    .child(Text::new(key).variant(ExtraTypography::MonoKey.role()).nowrap())
    .child(Text::new(title).variant(ExtraTypography::Strong.role()).nowrap())
    .child(Spacer::new().flex(1.0))
    .child(Text::new(step).variant(TypographyStyle::Description).nowrap())
    .into()
}
