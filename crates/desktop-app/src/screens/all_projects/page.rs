//! The page of All projects (`Column` `k4fycz` in `S5HDKf`): the heading with *Open project folder…* and *Add
//! project*, the *Projects* | *Usage* tabs, the error line of *Open project folder…* (`FV5pQ`), then the list
//! with what removing means under it, the empty list (`d1oNx8`) or why the list cannot be read (`T46trR`).
//! *Usage* is a page of its own (`rHA8G`): the heading and tabs pinned across the whole content panel, 40 from
//! its sides, over every project's usage, which pins its toolbar and scrolls its sections.

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
use orchester_workspace::{messages::Message, project_list::ListedProject};

use super::{
  Ask,
  folder::{self, OpenAction},
  row, screen_id,
};
use crate::{
  navigation::Navigation,
  routes::Route,
  theme::{ExtraSpacing, ExtraTypography, IconSize, ProjectsSpacing, ProjectsText, UsageSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    inspector::note,
    message::message_text,
    skeleton::{Skeleton, fading},
    state::EmptyState,
    tabs::{PageTab, page_tabs},
  },
};

/// The page's column (`k4fycz`).
const COLUMN_WIDTH: f32 = 1120.0;

/// What the page shows under its tabs.
pub(super) enum Body<'a> {
  /// The list is being read.
  Reading,
  /// The list cannot be read, and why.
  Failed(&'a Message),
  /// Every project was removed from the list.
  Empty,
  List {
    rows: &'a [ListedProject],
    ask: &'a Ask,
  },
  /// The *Usage* tab's page.
  Usage(Element),
}

/// Each project row's name and folder bones while the list is read (`EZ5Bq`).
const READING: [(f32, f32); 4] = [(120.0, 326.0), (98.0, 282.0), (140.0, 348.0), (110.0, 260.0)];

/// The page with `body` under its tabs, and the invites notice between them while it shows something.
pub(super) fn page(ctx: &mut Ctx, open: &OpenAction, body: Body<'_>, notice: Option<Element>) -> Element {
  let failure = folder::failure(ctx, open);
  let (state, content) = match body {
    Body::Reading => ("reading", reading(ctx)),
    Body::Failed(error) => ("failed", failed(ctx, error)),
    Body::Empty => ("empty", empty(ctx, open)),
    Body::List { rows, ask } => ("list", list(ctx, rows, ask)),
    Body::Usage(usage) => return usage_page(ctx, open, failure, usage),
  };
  let column = Column::new()
    .width(Dimension::Px(COLUMN_WIDTH))
    .spacing(ProjectsSpacing::Page.role())
    .child(heading(ctx, open))
    .child(tabs(ctx, false))
    .with_children(notice)
    .with_children(failure.map(|failure| error_line(&failure)))
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
fn usage_page(ctx: &mut Ctx, open: &OpenAction, failure: Option<Arc<str>>, usage: Element) -> Element {
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
    .child(heading(ctx, open))
    .child(tabs(ctx, true))
    .with_children(failure.map(|failure| error_line(&failure)));
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

/// *All projects*, what it lists, and the actions at the end (`Header` `t1fIB4`).
fn heading(ctx: &mut Ctx, open: &OpenAction) -> Row {
  let navigation = Navigation::of(ctx);
  let add = ActionButton::new("all-projects-add", ButtonKind::Primary, ctx.t("projects.add"))
    .icon(Icon::Plus)
    .opens(navigation.to(Route::AddProject(None)));
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::End)
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
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(folder::button(ctx, open, "all-projects-open-folder"))
        .child(add),
    )
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

/// Why *Open project folder…* opened nothing (`FV5pQ`).
fn error_line(failure: &str) -> Element {
  Row::new()
    .id("all-projects-error")
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
        .id("all-projects-error-text")
        .variant(ProjectsText::ErrorLine.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// The column headings, the rows, and what removing means (`Project list` `FJsmz`, `Note · removal` `phKj1`).
fn list(ctx: &mut Ctx, rows: &[ListedProject], ask: &Ask) -> Element {
  let last = rows.len().saturating_sub(1);
  let shown: Vec<Element> = rows
    .iter()
    .enumerate()
    .map(|(index, listed)| row::row(ctx, listed, ask, index == last))
    .collect();
  let table = Column::new()
    .id("project-list")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .child(row::headings(ctx))
    .with_children(shown);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ProjectsSpacing::Page.role())
    .child(table)
    .child(note(
      "all-projects-removal-note",
      Icon::Info,
      &ctx.t("projects.removal.note"),
    ))
    .into()
}

/// The list while it is read (`12 · All projects · loading` `EZ5Bq`): the column headings and what removing means stay;
/// each project's row is bones in its own cells, the lower rows fading. *Reading your projects…* (`projects.reading`)
/// names it.
fn reading(ctx: &mut Ctx) -> Element {
  let last = READING.len() - 1;
  let rows = READING
    .iter()
    .enumerate()
    .map(|(index, words)| row::bones(ctx, *words, index == last))
    .collect();
  let table = Column::new()
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .child(row::headings(ctx))
    .with_children(fading(rows));
  let bones = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ProjectsSpacing::Page.role())
    .child(table)
    .child(note(
      "all-projects-removal-note",
      Icon::Info,
      &ctx.t("projects.removal.note"),
    ));
  Skeleton::new("all-projects-loading", ctx.t("projects.reading")).show(ctx, bones)
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
