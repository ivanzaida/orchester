//! The page of All projects (`Column` `k4fycz` in `S5HDKf`; `P5z2w` in 09 `i5G19A`, `aPjYW` in 09b `dLOCJ`): the
//! heading with *Join a team*, *Open shared project…*, *Open project folder…* and *Add project*, the *Projects* |
//! *Usage* tabs, that an agent CLI is not found ([`super::agent_cli`]), the error line of *Open project folder…*
//! (`FV5pQ`), the team project offered (`ZyS3T`), then the list with what removing means under it and, with no team
//! joined, how to see a team's projects (`b3WTlF`), the empty list (`d1oNx8`) or why the list cannot be read
//! (`T46trR`). *Usage* is a page of its own (`rHA8G`): the heading and tabs pinned across the whole content panel, 40
//! from its sides, over every project's usage, which pins its toolbar and scrolls its sections.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ServerId;
use orchester_workspace::{messages::Message, project_list::ListedProject, sharing::Places};

use super::{
  Ask,
  folder::{self, OpenAction},
  row, screen_id,
};
use crate::{
  navigation::Navigation,
  routes::{OrchesterView, Route},
  theme::{ExtraSpacing, ExtraTypography, IconSize, ProjectsSpacing, ProjectsText, UsageSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    inspector::note,
    message::message_text,
    state::EmptyState,
    tabs::{PageTab, page_tabs},
  },
};

/// The page's column (`k4fycz`).
const COLUMN_WIDTH: f32 = 1120.0;
/// The line that says why *Open project folder…* opened nothing.
const OPEN_ERROR: &str = "all-projects-error";

/// What the heading's actions need: *Open project folder…*'s action, and the team servers joined (`None` until they
/// are read).
#[derive(Clone, Copy)]
pub(super) struct Heading<'a> {
  pub open: &'a OpenAction,
  pub joined: Option<&'a [ServerId]>,
}

/// What the page shows under its tabs.
pub(super) enum Body<'a> {
  /// The list cannot be read, and why.
  Failed(&'a Message),
  /// Every project was removed from the list.
  Empty,
  /// Nothing is on this computer, and an account's projects wait for its sign-in (`MvIpm`).
  NothingHere,
  List {
    rows: &'a [ListedProject],
    /// Where each project is, once it is read.
    places: Option<&'a Places>,
    /// Whether an account is signed in, so a row may offer a move.
    signed_in: bool,
    ask: &'a Ask,
  },
  /// The *Usage* tab's page.
  Usage(Element),
}

/// What the page shows between its tabs and its body, while each shows something.
#[derive(Default)]
pub(super) struct Above {
  /// What waits for a sign-in.
  pub waiting: Option<Element>,
  /// That an agent CLI is not found.
  pub cli: Option<Element>,
  /// The invites notice.
  pub notice: Option<Element>,
  /// The team project offered.
  pub offer: Option<Element>,
}

/// The page with `body` under its tabs, and between them what is `above` it.
pub(super) fn page(ctx: &mut Ctx, heading: Heading<'_>, body: Body<'_>, above: Above) -> Element {
  let failure = folder::failure(ctx, heading.open);
  let (state, content) = match body {
    Body::Failed(error) => ("failed", failed(ctx, error)),
    Body::Empty => ("empty", empty(ctx, heading.open)),
    Body::NothingHere => ("nothing-here", super::waiting::nothing_here(ctx)),
    Body::List {
      rows,
      places,
      signed_in,
      ask,
    } => ("list", list(ctx, (rows, places, signed_in), ask, heading.joined)),
    Body::Usage(usage) => return usage_page(ctx, heading, failure, usage),
  };
  let column = Column::new()
    .width(Dimension::Px(COLUMN_WIDTH))
    .spacing(ProjectsSpacing::Page.role())
    .child(header(ctx, heading))
    .child(tabs(ctx, false))
    .with_children(above.waiting)
    .with_children(above.cli)
    .with_children(above.notice)
    .with_children(failure.map(|failure| error_line(OPEN_ERROR, &failure)))
    .with_children(above.offer)
    .child(content);
  let scrolled = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Section))
    .align_items(Alignment::Center)
    .child(column);
  Column::new()
    .id(screen_id())
    .describe("state", state)
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(ScrollVertical::new(scrolled).id("all-projects-scroll").flex(1.0))
    .into()
}

/// The *Usage* tab (`yCCBS` in `rHA8G`): the heading and the tabs pinned at the top (`Top · pinned` `anSHC`:
/// 28 over them, 40 at the sides, 18 between them), then `usage`, which pins its toolbar and scrolls.
fn usage_page(ctx: &mut Ctx, heading: Heading<'_>, failure: Option<Arc<str>>, usage: Element) -> Element {
  let top = Column::new()
    .id("all-projects-top")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(UsageSpacing::Top.role())
        .left(UsageSpacing::Page.role())
        .right(UsageSpacing::Page.role()),
    )
    .spacing(ExtraSpacing::SettingsHead.role())
    .child(header(ctx, heading))
    .child(tabs(ctx, true))
    .with_children(failure.map(|failure| error_line(OPEN_ERROR, &failure)));
  Column::new()
    .id(screen_id())
    .describe("state", "usage")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(top)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .child(usage),
    )
    .into()
}

/// *All projects*, what it lists, and the actions at the end, 12 apart (`Header` `e9eYnG` in 09 `i5G19A`).
fn header(ctx: &mut Ctx, heading: Heading<'_>) -> Row {
  let navigation = Navigation::of(ctx);
  let (join, shared) = team_actions(ctx, heading.joined);
  let add = ActionButton::new("all-projects-add", ButtonKind::Primary, ctx.t("projects.add"))
    .icon(Icon::Plus)
    .opens(navigation.to(Route::AddProject(None)));
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Snug.role())
        .child(
          Text::new(&ctx.t("projects.title"))
            .id("all-projects-title")
            .variant(ExtraTypography::Display.role()),
        )
        .child(Text::new(&ctx.t("projects.description")).variant(ProjectsText::Sub.role())),
    )
    .child(
      Row::new()
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(join)
        .child(shared)
        .child(folder::button(ctx, heading.open, "all-projects-open-folder"))
        .child(add),
    )
}

/// *Join a team* (`JL7DS`) and *Open shared project…* (`sK0yA`). Joining is Settings › Orchester Cloud's, until the
/// start page's own steps are built; a shared project opens from its team's page there, which lists the team's
/// projects with *Link a checkout…*: the one team's when exactly one is joined, otherwise the teams to choose from.
fn team_actions(ctx: &mut Ctx, joined: Option<&[ServerId]>) -> (ActionButton, ActionButton) {
  let navigation = Navigation::of(ctx);
  let teams = Route::Orchester(OrchesterView::default());
  let shared = match joined {
    Some([server]) => Route::Orchester(OrchesterView::server(*server)),
    _ => teams.clone(),
  };
  let join = ActionButton::new(
    "all-projects-join-team",
    ButtonKind::Secondary,
    ctx.t("projects.header.joinTeam"),
  )
  .icon(Icon::Users)
  .opens(navigation.to(teams));
  let shared = ActionButton::new(
    "all-projects-open-shared",
    ButtonKind::Secondary,
    ctx.t("projects.header.openShared"),
  )
  .icon(Icon::FolderGit2)
  .opens(navigation.to(shared));
  (join, shared)
}

/// *Projects* | *Usage* (`Tabs` `Uecqx`), `usage` the current one or not.
fn tabs(ctx: &mut Ctx, usage: bool) -> Element {
  let navigation = Navigation::of(ctx);
  let projects = PageTab::to(
    "all-projects-tab-projects",
    ctx.t("projects.tabs.projects"),
    !usage,
    navigation.to(Route::AllProjects),
  );
  let usage = PageTab::to(
    "all-projects-tab-usage",
    ctx.t("projects.tabs.usage"),
    usage,
    navigation.to(Route::Usage(Box::default())),
  );
  page_tabs(ctx, "all-projects-tabs", [projects, usage]).into()
}

/// Why an action of the page did nothing (`FV5pQ`).
pub(super) fn error_line(id: &str, failure: &str) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Inset.role(),
      ExtraSpacing::Loose.role(),
    ))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(icon(Icon::CircleAlert, IconSize::Lg, PaletteColor::TextSecondary))
    .child(
      Text::new(failure)
        .id(format!("{id}-text"))
        .variant(ProjectsText::ErrorLine.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// The column headings, the rows, what removing means (`Project list` `FJsmz`, `Note · removal` `phKj1`) and, once it
/// is read that no team is joined, how to see a team's projects (`Hint` `b3WTlF` in 09b `dLOCJ`).
fn list(
  ctx: &mut Ctx,
  (rows, places, signed_in): (&[ListedProject], Option<&Places>, bool),
  ask: &Ask,
  joined: Option<&[ServerId]>,
) -> Element {
  let last = rows.len().saturating_sub(1);
  let shown: Vec<Element> = rows
    .iter()
    .enumerate()
    .map(|(index, listed)| {
      let place = places.and_then(|places| places.of(listed.project.id));
      row::row(ctx, (listed, place, signed_in), ask, index == last)
    })
    .collect();
  let table = Column::new()
    .id("project-list")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .child(row::headings(ctx))
    .with_children(shown);
  let hint = joined
    .is_some_and(<[ServerId]>::is_empty)
    .then(|| note("all-projects-team-hint", Icon::Info, &ctx.t("projects.header.teamHint")));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ProjectsSpacing::Page.role())
    .child(table)
    .child(note(
      "all-projects-removal-note",
      Icon::Info,
      &ctx.t("projects.removal.note"),
    ))
    .with_children(hint)
    .into()
}

/// No projects in the list, with the heading's actions again (`State · No projects in the list` `d1oNx8`).
fn empty(ctx: &mut Ctx, open: &OpenAction) -> Element {
  let navigation = Navigation::of(ctx);
  let add = ActionButton::new("all-projects-empty-add", ButtonKind::Primary, ctx.t("projects.add"))
    .icon(Icon::Plus)
    .opens(navigation.to(Route::AddProject(None)));
  EmptyState::new(
    "all-projects-empty",
    Icon::Folders,
    ctx.t("projects.empty.title"),
    ctx.t("projects.empty.summary"),
  )
  .wide()
  .action(add)
  .action(folder::button(ctx, open, "all-projects-empty-open-folder"))
  .into()
}

/// The list cannot be read; nothing else is shown and nothing changed (`State · The projects could not be
/// read` `T46trR`).
fn failed(ctx: &Ctx, error: &Message) -> Element {
  EmptyState::new(
    "all-projects-failed",
    Icon::CircleAlert,
    ctx.t("projects.loadFailed"),
    message_text(ctx, error),
  )
  .wide()
  .into()
}
