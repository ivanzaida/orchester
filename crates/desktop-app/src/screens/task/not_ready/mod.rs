//! The card of a run that did not start because its pipeline is not ready on this computer (`Run state · Not ready
//! here` `B6ee9` in R1 `Ku0qu` for a team project, `Run state · Not ready` in L9 `f5CEX` for a local one): at the top
//! of the Summary, a head that says nothing started, one line of what to do, a row for each part that is missing with
//! its fix, and *Open Agents*. The words name a team and a computer only in a team project. Nothing was queued, claimed
//! or counted as an attempt, and the card goes once a start goes through. Each fix opens the place the person sets that
//! one thing ([`fix`]).

mod fix;
mod notice;
mod reread;
mod words;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::time::Timestamp;
use orchester_workspace::{
  readiness::{NotReadyHere, NotReadyPart},
  team_work::ClaimEnd,
};

pub use self::notice::notice;
pub(super) use self::reread::{CardProps, missing_now};
use self::words::{Glyph, Words};
use super::{
  TaskContext,
  header::controls::{RunControls, RunRefused},
  team_controls,
};
use crate::{
  navigation::{Navigation, link::Open},
  routes::{ProjectPage, Route},
  team::start::{Started, Stop},
  theme::{CardsText, ExtraSpacing, ExtraTypography, IconSize, NOT_READY_ROW_HEIGHT, TaskRadius, TaskText},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    icon::{Icon, icon},
    pressable::opening,
  },
};

/// What the person's last start was refused for, if it was because the task's pipeline is not ready here: a team
/// task's start through its server, or a local project's run control.
pub fn refused(ctx: &mut Ctx, task: &TaskContext) -> Option<NotReadyHere> {
  if task.team.is_some() {
    return match team_controls::start_standing(ctx, task)?.started {
      Started::Stopped(Stop::NotReadyHere(refusal)) => Some(refusal),
      _ => None,
    };
  }
  match RunControls::of(ctx).action.state().get().error? {
    RunRefused::NotReadyHere(refusal) => Some(refusal),
    RunRefused::Message(_) => None,
  }
}

/// When the person pressed the Run that [`refused`] says was refused.
pub fn pressed(ctx: &mut Ctx, task: &TaskContext) -> Option<Timestamp> {
  if task.team.is_some() {
    return TeamControls::of(ctx)?.pressed.get().map(|pressed| pressed.0);
  }
  RunControls::of(ctx).requested.get().map(|requested| requested.at)
}

/// The card the Summary shows for the last refusal of `task`'s start, if it was for not being ready here. The card
/// asks again when it is shown and when the connections, tools or profiles change ([`reread`]), so it follows a person
/// who fixed what it lists and came back.
pub fn shown(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let refusal = refused(ctx, task)?;
  Some(ctx.mount_keyed::<reread::NotReadyCard>(
    "task-not-ready-card",
    reread::CardProps {
      task: task.clone(),
      refusal,
    },
  ))
}

/// The card of `refusal` for `task`.
fn card_of(ctx: &mut Ctx, task: &TaskContext, refusal: &NotReadyHere) -> Element {
  let team = task.team.is_some();
  let project = task.project.open.project.id();
  let navigation = Navigation::of(ctx);
  let open = |page: ProjectPage| navigation.to(Route::Project(project, page));
  let rows = refusal
    .parts()
    .iter()
    .enumerate()
    .map(|(index, part)| row(ctx, (index, part), (team, index + 1 == refusal.parts().len()), &open))
    .collect::<Vec<_>>();
  let body = match (team, refusal.claim_end()) {
    (_, Some(ClaimEnd::Released)) => "summary.notReady.bodyAfterStart",
    (_, Some(ClaimEnd::Kept)) => "summary.notReady.bodyAfterStartKept",
    (_, Some(ClaimEnd::Held)) => "summary.notReady.bodyAfterStartHeld",
    (true, None) => "summary.notReady.bodyTeam",
    (false, None) => "summary.notReady.body",
  };
  card("task-not-ready", CardSurface::Attention)
    .rounded(TaskRadius::NotReadyCard.role())
    .spacing(SpacingSize::Md)
    .child(head(ctx, refusal, team))
    .child(
      Text::new(&ctx.t(body))
        .id("task-not-ready-body")
        .variant(ExtraTypography::Lead.role()),
    )
    .child(rows_box(rows))
    .child(actions(ctx, team, open(ProjectPage::Agents)))
    .into()
}

/// The glyph, what happened and how much is missing.
fn head(ctx: &Ctx, refusal: &NotReadyHere, team: bool) -> Row {
  let title = match (team, refusal.after_server_started()) {
    (_, true) => "summary.notReady.titleAfterStart",
    (true, false) => "summary.notReady.titleTeam",
    (false, false) => "summary.notReady.title",
  };
  let count = refusal.parts().len();
  let meta = match (count == 1, team) {
    (true, true) => "summary.notReady.metaOneTeam",
    (true, false) => "summary.notReady.metaOne",
    (false, true) => "summary.notReady.metaOtherTeam",
    (false, false) => "summary.notReady.metaOther",
  };
  let pipeline = match refusal.number() {
    Some(number) => ctx.t_args(
      "summary.properties.pipelineValue",
      [
        ("pipeline", refusal.pipeline().to_owned()),
        ("version", number.to_string()),
      ],
    ),
    None => refusal.pipeline().into(),
  };
  let meta = ctx.t_args(
    meta,
    [("pipeline", pipeline.as_ref()), ("count", count.to_string().as_str())],
  );
  Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .spacing(SpacingSize::Sm)
    .child(icon(Icon::CircleAlert, IconSize::Md, PaletteColor::Warning))
    .child(
      Text::new(&ctx.t(title))
        .id("task-not-ready-title")
        .variant(TaskText::SectionTitle.role()),
    )
    .child(
      Text::new(&meta)
        .id("task-not-ready-meta")
        .variant(TypographyStyle::Description),
    )
}

/// The rows in one outline.
fn rows_box(rows: Vec<Element>) -> Column {
  Column::new()
    .id("task-not-ready-rows")
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .clip()
    .with_children(rows)
}

/// One part: its glyph, what it is about, what is missing in the warning color and its fix at the end.
fn row(
  ctx: &mut Ctx,
  (index, part): (usize, &NotReadyPart),
  (team, last): (bool, bool),
  open: &dyn Fn(ProjectPage) -> Open,
) -> Element {
  let Words { glyph, what, need } = words::of(ctx, part, team);
  let id = format!("task-not-ready-row-{index}");
  let glyph: Element = match glyph {
    Glyph::Icon(glyph) => icon(glyph, IconSize::Md, PaletteColor::Warning).into(),
    Glyph::Mark(mark) => mark.render(ctx, IconSize::Md),
  };
  let mut row = Row::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(NOT_READY_ROW_HEIGHT))
    .align_items(Alignment::Center)
    .spacing(ExtraSpacing::Loose.role())
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Snug.role()))
    .child(glyph)
    .child(
      Text::new(&what)
        .id(format!("{id}-what"))
        .variant(CardsText::Decided.role()),
    )
    .child(
      Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(
        Text::new(&need)
          .id(format!("{id}-need"))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::Warning),
      ),
    );
  if !last {
    row = row.border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border));
  }
  if let Some((fix, page)) = fix::of(part) {
    let label = ctx.t_args("summary.notReady.fix.arrow", [("fix", ctx.t(fix.key()).as_ref())]);
    row = row.child(
      opening(open(page))
        .id(format!("{id}-fix"))
        .child(Text::new(&label).variant(TypographyStyle::Button)),
    );
  }
  row.into()
}

/// *Open Agents*, where everything this computer lacks is listed, and what the fixes do.
fn actions(ctx: &Ctx, team: bool, agents: Open) -> Element {
  let (button, note) = if team {
    ("summary.notReady.openAgentsTeam", "summary.notReady.footnoteTeam")
  } else {
    ("summary.notReady.openAgents", "summary.notReady.footnote")
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .spacing(SpacingSize::Sm)
    .child(
      ActionButton::new("task-not-ready-open-agents", ButtonKind::Secondary, ctx.t(button))
        .icon(Icon::Bot)
        .opens(agents),
    )
    .child(Column::new().flex(1.0))
    .child(
      Text::new(&ctx.t(note))
        .id("task-not-ready-note")
        .variant(TypographyStyle::Description),
    )
    .into()
}
