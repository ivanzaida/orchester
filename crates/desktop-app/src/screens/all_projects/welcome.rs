//! The welcome of a first launch (`Content · Welcome` `MANaM` in `UQZRa`): the mark, what Orchester is for,
//! *Create your first project* and *Open project folder…*, the three steps to a first task, that an agent CLI is not
//! found while none is ([`super::agent_cli`]), and a footer that says each project keeps its own instructions, tools,
//! secrets and connections. Nothing is made up: no recent project, no activity, no runner.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  folder::{self, OpenAction},
  screen_id,
};
use crate::{
  brand,
  navigation::Navigation,
  routes::Route,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, ProjectsSpacing, ProjectsText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// The introduction and the steps side by side (`Inner` `UvN35`), the introduction (`I5Dadl`), its lead
/// (`aFsI6`) and the footer (`N3VzU`).
const INNER_WIDTH: f32 = 1120.0;
const INTRODUCTION_WIDTH: f32 = 520.0;
const LEAD_WIDTH: f32 = 470.0;
const FOOTER_HEIGHT: f32 = 56.0;
/// The mark's tile (`Mark` `B0sYBM`) and a step's glyph tile (`Glyph` `HnSFC`).
const MARK_TILE: f32 = 40.0;
const STEP_TILE: f32 = 32.0;
/// The extra room the introduction leaves under the mark and above the actions (`Gap` `U9MyI`, `eeGRG`), and
/// under the steps' heading (`cezXL`).
const MARK_GAP: f32 = 4.0;
const ACTIONS_GAP: f32 = 6.0;

/// The welcome, with that an agent CLI is not found under its introduction and steps when `cli` says so.
pub(super) fn welcome(ctx: &mut Ctx, open: &OpenAction, cli: Option<Element>) -> Element {
  let inner = Row::new()
    .width(Dimension::Px(INNER_WIDTH))
    .spacing(ProjectsSpacing::WelcomeColumns.role())
    .align_items(Alignment::Center)
    .child(introduction(ctx, open))
    .child(steps(ctx));
  let main = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ProjectsSpacing::WelcomeEdge.role(),
      ProjectsSpacing::Wide.role(),
    ))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(inner);
  let main: Element = match cli {
    None => main.flex(1.0).into(),
    // It does not fit at the window's smallest size with the warning, so only then the page scrolls; at least as high
    // as the window, it stays centred.
    Some(cli) => ScrollVertical::new(
      main
        .min_height(Dimension::Pct(100.0))
        .spacing(SpacingSize::Section)
        .child(
          Column::new()
            .width(Dimension::Pct(100.0))
            .max_width(Dimension::Px(INNER_WIDTH))
            .child(cli),
        ),
    )
    .id("welcome-scroll")
    .flex(1.0)
    .into(),
  };
  Column::new()
    .id(screen_id())
    .describe("state", "welcome")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(main)
    .child(footer(ctx))
    .into()
}

fn introduction(ctx: &mut Ctx, open: &OpenAction) -> Column {
  let mark = Row::new()
    .width(Dimension::Px(MARK_TILE))
    .height(Dimension::Px(MARK_TILE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(RadiusSize::Lg)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(brand::mark());
  let navigation = Navigation::of(ctx);
  let create = ActionButton::new(
    "welcome-create-project",
    ButtonKind::Primary,
    ctx.t("projects.welcome.createFirst"),
  )
  .icon(Icon::Plus)
  .large()
  .opens(navigation.to(Route::AddProject(None)));
  let actions = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(create)
    .child(folder::button(ctx, open, "welcome-open-folder").large());
  Column::new()
    .id("welcome-introduction")
    .width(Dimension::Px(INTRODUCTION_WIDTH))
    .spacing(SpacingSize::Lg)
    .child(mark)
    .child(gap(MARK_GAP))
    .child(
      Text::new(&ctx.t("projects.welcome.eyebrow"))
        .variant(ProjectsText::Eyebrow.role())
        .nowrap(),
    )
    .child(
      Text::new(&ctx.t("projects.welcome.title"))
        .id("welcome-title")
        .variant(ProjectsText::WelcomeTitle.role()),
    )
    .child(
      Text::new(&ctx.t("projects.welcome.lead"))
        .variant(ProjectsText::WelcomeLead.role())
        .width(Dimension::Px(LEAD_WIDTH)),
    )
    .child(gap(ACTIONS_GAP))
    .child(actions)
    .child(
      Text::new(&ctx.t("projects.welcome.next"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
}

/// Room of `height` between two parts of a column, besides its gap.
fn gap(height: f32) -> Row {
  Row::new().height(Dimension::Px(height))
}

/// *THREE STEPS TO YOUR FIRST TASK* beside a line (`Steps` `RZyxY`).
fn steps(ctx: &Ctx) -> Column {
  let all = [
    ("01", Icon::FolderGit2, "repository"),
    ("02", Icon::Bot, "agents"),
    ("03", Icon::ListChecks, "task"),
  ];
  let last = all.len() - 1;
  let rows = all.into_iter().enumerate().map(|(index, (number, glyph, key))| {
    step(
      (number, glyph),
      &ctx.t(&format!("projects.welcome.steps.{key}")),
      &ctx.t(&format!("projects.welcome.steps.{key}Summary")),
      index == last,
    )
  });
  Column::new()
    .id("welcome-steps")
    .flex(1.0)
    .padding(
      Padding::new()
        .top(SpacingSize::Xs)
        .bottom(SpacingSize::Xs)
        .left(ProjectsSpacing::Wide.role()),
    )
    .spacing(SpacingSize::Xs)
    .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&ctx.t("projects.welcome.stepsHeading"))
        .variant(ProjectsText::Eyebrow.role())
        .nowrap(),
    )
    .child(gap(ACTIONS_GAP))
    .with_children(rows)
}

/// A step: its number, its glyph on a tile, its title and what it means (`Step · Connect a repository` `katB2`).
fn step((number, glyph): (&str, Icon), title: &str, summary: &str, last: bool) -> Element {
  let tile = Row::new()
    .width(Dimension::Px(STEP_TILE))
    .height(Dimension::Px(STEP_TILE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(ExtraRadius::Tile.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Nav, PaletteColor::TextSecondary));
  let text = Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Fine.role())
    .child(Text::new(title).variant(ProjectsText::Heading.role()))
    .child(Text::new(summary).variant(ExtraTypography::Prose.role()));
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Lg))
    .spacing(ExtraSpacing::Inset.role())
    .align_items(Alignment::Start)
    .child(Text::new(number).variant(ProjectsText::StepNumber.role()))
    .child(tile)
    .child(text);
  if last {
    row.into()
  } else {
    row
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .into()
  }
}

/// That each project keeps its own, and that none is listed yet (`Footer` `N3VzU`).
fn footer(ctx: &Ctx) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FOOTER_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Section))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::FolderLock, IconSize::Lg, PaletteColor::TextSecondary))
    .child(Text::new(&ctx.t("projects.welcome.scope")).variant(TypographyStyle::Description))
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&ctx.t("projects.welcome.noProjects")).variant(TypographyStyle::Caption))
}
