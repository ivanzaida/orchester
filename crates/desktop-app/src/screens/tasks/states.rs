//! What the list shows instead of rows. A project without tasks says so in the middle of the panel, with New task and
//! Import resources (`Empty` `GQZ6J` in `03 No tasks yet` `f54MO`: a 40 tile, `No tasks yet` in 15/600 and one
//! sentence in 12.5 `text_secondary`, 6 apart, the actions 16 under them); while the tasks are read, the loader
//! (`Reading` `O8pgq`); when they cannot be read, why, with Try again (`Notice · Tasks cannot be read` `kXWCj`); a view
//! whose rows the search or the filters hide says so, with Clear (`No matches` `WaehI`); a view without tasks says
//! which tasks it shows. Loading and a failure never look like an empty list. Over Active's rows, or where they would
//! be, a notice names what waits and is no task's, which the list cannot show, and opens the Needs you panel that lists
//! it.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  messages::Message,
  tasks::{attention, data::ProjectTasks},
};

use super::header::new_task;
use crate::{
  navigation::Navigation,
  project::ProjectFeed,
  routes::{ImportTab, ProjectPage, Route, TaskListTab},
  shell::needs_you::PanelOpen,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, TasksSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    notice::{Notice, NoticeTone},
    state::{load_error, loading},
  },
};

/// The empty project's glyph tile and the column its words keep to (`Glyph` `HaMgD`, `Column` `u7ia7g`); the tile of a
/// view without rows (`ngIiE`).
const EMPTY_TILE: f32 = 40.0;
const EMPTY_COLUMN: f32 = 420.0;
const NO_ROWS_TILE: f32 = 36.0;

/// The project has no task yet: New task, or Import resources from another project.
pub(super) fn empty_project(ctx: &mut Ctx, project: ProjectId) -> Element {
  let navigation = Navigation::of(ctx);
  let import = ActionButton::new("tasks-empty-import", ButtonKind::Secondary, ctx.t("tasks.empty.import"))
    .icon(Icon::Download)
    .opens(navigation.to(Route::Project(project, ProjectPage::Import(ImportTab::All))));
  let words = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("tasks.empty.title"))
        .id("tasks-empty-title")
        .variant(TypographyStyle::Title),
    )
    .child(
      Text::new(&ctx.t("tasks.empty.detail"))
        .id("tasks-empty-detail")
        .width(Dimension::Pct(100.0))
        .text_align(lurq::layout::text_style::TextAlign::Center)
        .variant(ExtraTypography::Lead.role()),
    );
  let column = Column::new()
    .width(Dimension::Px(EMPTY_COLUMN))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::Center)
    .child(tile(Icon::ListChecks, EMPTY_TILE))
    .child(words)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(new_task(ctx, "tasks-empty-new-task", project, &navigation))
        .child(import),
    );
  Column::new()
    .id("tasks-empty")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(Padding::new().bottom(TasksSpacing::EmptyLift.role()))
    .justify(Justify::Center)
    .align_items(Alignment::Center)
    .describe("state", "empty")
    .child(column)
    .into()
}

/// The loader, saying whose tasks are read.
pub(super) fn reading(ctx: &Ctx, project: &str) -> Row {
  loading("tasks-loading", &ctx.t_args("tasks.loading", [("project", project)]))
}

/// The tasks cannot be read, why, and Try again, which reads them again.
pub(super) fn failed(ctx: &Ctx, feed: &ProjectFeed, error: &Message) -> Element {
  let body = ctx.t_args("tasks.loadError.body", [("reason", message_text(ctx, error))]);
  let feed = feed.clone();
  // Reading the tasks again is what a save asks for too.
  load_error(
    ctx,
    "tasks-load-error",
    (&ctx.t("tasks.loadError.title"), &body),
    move || {
      feed.saved();
    },
  )
}

/// The search or the filters hide all `count` tasks of `view`; Clear undoes them.
pub(super) fn no_matches(ctx: &Ctx, (view, count): (&str, usize), clear: impl Fn() + Send + Sync + 'static) -> Column {
  let key = if count == 1 {
    "tasks.noMatches.inView.one"
  } else {
    "tasks.noMatches.inView.other"
  };
  let detail = ctx.t_args(key, [("view", view.to_owned()), ("count", count.to_string())]);
  let clear = ActionButton::new(
    "tasks-clear-query",
    ButtonKind::Secondary,
    ctx.t("tasks.noMatches.clear"),
  )
  .icon(Icon::X)
  .on_click(clear);
  no_rows((Icon::SearchX, &ctx.t("tasks.noMatches.title"), &detail), Some(clear))
}

/// `view` holds no task.
pub(super) fn empty_view(ctx: &Ctx, view: TaskListTab) -> Column {
  let detail = ctx.t(match view {
    TaskListTab::Active => "tasks.emptyView.active",
    TaskListTab::Backlog => "tasks.emptyView.backlog",
    TaskListTab::Done => "tasks.emptyView.done",
    TaskListTab::All => "tasks.emptyView.all",
  });
  no_rows((Icon::ListChecks, &ctx.t("tasks.emptyView.title"), &detail), None)
}

/// On Active, whose first group is Needs you, what waits for the person in `tasks` and is no task's
/// ([`attention::outside_tasks`]), with *Open Needs you*, which opens the title bar's panel that lists it; `None` in
/// another view, or when everything that waits is a task's.
pub(super) fn outside_tasks(ctx: &mut Ctx, view: TaskListTab, tasks: &ProjectTasks) -> Option<Element> {
  if view != TaskListTab::Active {
    return None;
  }
  let title = attention::outside_tasks(tasks)?;
  let panel = PanelOpen::of(ctx);
  let open = ActionButton::new(
    "tasks-outside-tasks-open",
    ButtonKind::Secondary,
    ctx.t("tasks.outsideTasks.open"),
  )
  .on_click(move || panel.open());
  Some(
    Notice::new("tasks-outside-tasks", NoticeTone::Info, message_text(ctx, &title))
      .icon(Icon::Hand)
      .body(ctx.t("tasks.outsideTasks.body"))
      .action(open)
      .into(),
  )
}

/// The glyph, why there is no row, and what can be done about it.
fn no_rows((glyph, title, detail): (Icon, &str, &str), action: Option<ActionButton>) -> Column {
  Column::new()
    .id("tasks-no-rows")
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Loading.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(tile(glyph, NO_ROWS_TILE))
    .child(
      Row::new().padding(Padding::new().top(SpacingSize::Xs)).child(
        Text::new(title)
          .id("tasks-no-rows-title")
          .variant(ExtraTypography::Strong.role()),
      ),
    )
    .child(
      Text::new(detail)
        .id("tasks-no-rows-detail")
        .variant(TypographyStyle::Description),
    )
    .with_children(action.map(|action| {
      Row::new()
        .padding(Padding::new().top(ExtraSpacing::Snug.role()))
        .child(action)
    }))
}

/// `glyph` on a tile `side` square (`HaMgD`, `ngIiE`).
fn tile(glyph: Icon, side: f32) -> Row {
  Row::new()
    .width(Dimension::Px(side))
    .height(Dimension::Px(side))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(ExtraRadius::Tile.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Xl, PaletteColor::TextSecondary))
}
