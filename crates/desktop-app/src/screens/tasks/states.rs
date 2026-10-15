//! What the list shows instead of rows. A project without tasks says so, with New task (`Empty project`
//! `LeIOm`); while the tasks are read, the table's header stands over the loader (`Reading` `O8pgq`); when
//! they cannot be read, why, with Try again (`Notice · Tasks cannot be read` `kXWCj`); and a tab whose
//! rows the search or the pipeline filter hide says so, with Clear (`No matches` `WaehI`). Loading and a
//! failure never look like an empty list. Under the search and over the Needs you tab's rows, or where they would
//! be, a notice names what waits and is no task's, which the list cannot show, and opens the Needs you panel that
//! lists it (where `Epiq5` and `oDyZB` put a notice).

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

use super::{header::new_task, table};
use crate::{
  navigation::Navigation,
  project::ProjectFeed,
  routes::TaskListTab,
  shell::needs_you::PanelOpen,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    notice::{Notice, NoticeTone},
    state::{EmptyState, load_error, loading},
  },
};

/// `Icon` `ngIiE` of `No matches`.
const GLYPH_TILE: f32 = 36.0;

/// The project has no task yet: New task. The design's *Ready to run* checklist under it (`Ready` `HiAa2`:
/// agents, pipeline, connection) is deferred, an agreed deviation: nothing reads that readiness yet.
pub(super) fn empty_project(ctx: &mut Ctx, project: ProjectId) -> Element {
  let navigation = Navigation::of(ctx);
  EmptyState::new(
    "tasks-empty",
    Icon::ListPlus,
    ctx.t("tasks.empty.title"),
    ctx.t("tasks.empty.detail"),
  )
  .action(new_task(ctx, "tasks-empty-new-task", project, navigation))
  .into()
}

/// The table's header over the loader, saying whose tasks are read.
pub(super) fn reading(ctx: &Ctx, project: &str) -> Column {
  let what = ctx.t_args("tasks.loading", [("project", project)]);
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(table::header(ctx))
    .child(loading("tasks-loading", &what))
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

/// The search or the pipeline filter hide all `count` tasks of `tab`; Clear undoes both.
pub(super) fn no_matches(ctx: &Ctx, (tab, count): (&str, usize), clear: impl Fn() + Send + Sync + 'static) -> Column {
  let key = if count == 1 {
    "tasks.noMatches.inTab.one"
  } else {
    "tasks.noMatches.inTab.other"
  };
  let detail = ctx.t_args(key, [("tab", tab.to_owned()), ("count", count.to_string())]);
  let clear = ActionButton::new(
    "tasks-clear-query",
    ButtonKind::Secondary,
    ctx.t("tasks.noMatches.clear"),
  )
  .icon(Icon::X)
  .on_click(clear);
  no_rows(
    ctx,
    (Icon::SearchX, &ctx.t("tasks.noMatches.title"), &detail),
    Some(clear),
  )
}

/// `tab` holds no task.
pub(super) fn empty_tab(ctx: &Ctx, tab: &str, needs_you: bool) -> Column {
  let detail = if needs_you {
    ctx.t("tasks.emptyNeedsYou.detail")
  } else {
    ctx.t_args("tasks.emptyTab.detail", [("tab", tab)])
  };
  no_rows(ctx, (Icon::ListChecks, &ctx.t("tasks.emptyTab.title"), &detail), None)
}

/// On the Needs you tab, what waits for the person in `tasks` and is no task's ([`attention::outside_tasks`]),
/// with *Open Needs you*, which opens the title bar's panel that lists it; `None` on another tab, or when
/// everything that waits is a task's.
pub(super) fn outside_tasks(ctx: &mut Ctx, tab: TaskListTab, tasks: &ProjectTasks) -> Option<Element> {
  if tab != TaskListTab::NeedsYou {
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

/// The table's header over the glyph, why there is no row, and what can be done about it.
fn no_rows(ctx: &Ctx, (glyph, title, detail): (Icon, &str, &str), action: Option<ActionButton>) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(table::header(ctx))
    .child(
      Column::new()
        .id("tasks-no-rows")
        .width(Dimension::Pct(100.0))
        .padding(Padding::vertical(ExtraSpacing::Loading.role()))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(tile(glyph))
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
        })),
    )
}

/// The glyph on its tile (`ngIiE`).
fn tile(glyph: Icon) -> Row {
  Row::new()
    .width(Dimension::Px(GLYPH_TILE))
    .height(Dimension::Px(GLYPH_TILE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(ExtraRadius::Tile.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Xl, PaletteColor::TextSecondary))
}
