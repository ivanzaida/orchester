//! Project overview (`Twtqc`): the runner connection notice, the heading with New task, and
//! the project's newest tasks (`BPSF0`), or what the project is while it has none. Metrics,
//! attention and activity have no data yet and are left out.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::time::Timestamp;

use super::{ProjectNav, Section, TasksState, task_rows::list_row};
use crate::{
  display,
  tasks::{data::ProjectTasks, list},
  theme::{ExtraColor, ExtraSpacing},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    message_text,
    panel::RecoveryPanel,
    pressable::{pressable, surface},
    property::property,
    tab::{Tab, tab_row},
  },
  workspace::OpenProject,
};

/// Project bar height (`CsB3h`) and New task button height (`nMeVq`).
const PROJECT_BAR_HEIGHT: f32 = 64.0;
const NEW_TASK_HEIGHT: f32 = 38.0;
/// Longest repository path shown in full in the overview panel.
const PATH_CHARS: usize = 48;
/// Task list heading, tab row, tab and footer heights (`aZa78`, `i2uFe`, `SOW6S`, `WV44u`).
const LIST_HEADING_HEIGHT: f32 = 34.0;
const TAB_ROW_HEIGHT: f32 = 48.0;
const TAB_HEIGHT: f32 = 38.0;
const LIST_FOOTER_HEIGHT: f32 = 54.0;
/// Tasks shown in the overview; the task list has all of them.
const RECENT_TASKS: usize = 6;

/// `notice` is the runner connection notice, if the runner needs attention.
pub fn overview(
  ctx: &Ctx,
  open: &OpenProject,
  notice: Option<Element>,
  tasks: &TasksState,
  nav: &ProjectNav,
) -> Element {
  let body = match tasks {
    TasksState::Loading => Column::new().into(),
    TasksState::Failed(error) => {
      RecoveryPanel::new(Icon::AlertCircle, &ctx.t("tasks.loadFailed"), &message_text(ctx, error)).into()
    }
    TasksState::Ready(tasks) if tasks.tasks.is_empty() => empty_overview(ctx, open),
    TasksState::Ready(tasks) => task_list(ctx, tasks, nav),
  };
  // The overview body spaces its sections by 28 (`FMRs3`), by 24 below a connection notice
  // (`G8JTc`, `c7qiCh`, `s3EdDI`).
  let spacing = if notice.is_some() {
    SpacingSize::Lg
  } else {
    ExtraSpacing::Rail.role()
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(project_bar(ctx, open))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .padding(SpacingSize::Section)
        .spacing(spacing)
        .with_children(notice)
        .child(heading(ctx, open, nav))
        .child(body),
    )
    .into()
}

fn project_bar(ctx: &Ctx, open: &OpenProject) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(PROJECT_BAR_HEIGHT))
    .padding_horizontal(SpacingSize::Section)
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(Text::new(open.project.name().as_str()).variant(TypographyStyle::Description))
    .child(icon(Icon::ChevronRight, IconSize::Small, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("nav.overview"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary),
    )
    .into()
}

fn heading(ctx: &Ctx, open: &OpenProject, nav: &ProjectNav) -> Element {
  let nav = nav.clone();
  Row::new()
    .width(Dimension::Pct(100.0))
    .justify(Justify::SpaceBetween)
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .spacing(SpacingSize::Sm)
        .child(Text::new(open.project.name().as_str()).variant(TypographyStyle::Heading))
        .child(
          Text::new(&ctx.t("overview.description"))
            .variant(TypographyStyle::Body)
            .color(PaletteColor::TextSecondary),
        ),
    )
    .child(
      ActionButton::new(ctx.t("overview.newTask"))
        .primary()
        .icon(Icon::Plus)
        .id("new-task")
        .height(Dimension::Px(NEW_TASK_HEIGHT))
        .roomy()
        .on_click(move || nav.show(Section::NewTask))
        .render(ctx),
    )
    .into()
}

/// The newest tasks (`BPSF0`). Only All tasks can be told apart today; the other tabs have
/// no runs to count and are unavailable.
fn task_list(ctx: &Ctx, tasks: &ProjectTasks, nav: &ProjectNav) -> Element {
  let now = Timestamp::now();
  let rows = list::recent(&tasks.tasks, &tasks.pipelines, &tasks.phases, RECENT_TASKS);
  let pipeline_filter = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .describe("state", "unavailable")
    .cursor(CursorIcon::NotAllowed)
    .child(Text::new(&ctx.t("tasks.filters.allPipelines")).variant(TypographyStyle::Caption))
    .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextMuted));
  let tabs = tab_row(
    Dimension::Px(TAB_ROW_HEIGHT),
    SpacingSize::Sm,
    None,
    [
      Tab::new("overview-tab-all", ctx.t("tasks.tabs.all"), Dimension::Px(TAB_HEIGHT))
        .count(Some(tasks.tasks.len()))
        .selected(true),
      Tab::new(
        "overview-tab-running",
        ctx.t("overview.tabs.running"),
        Dimension::Px(TAB_HEIGHT),
      ),
      Tab::new(
        "overview-tab-needs-you",
        ctx.t("overview.tabs.needsYou"),
        Dimension::Px(TAB_HEIGHT),
      ),
      Tab::new(
        "overview-tab-queued",
        ctx.t("overview.tabs.queued"),
        Dimension::Px(TAB_HEIGHT),
      ),
    ],
  );
  Column::new()
    .id("overview-tasks")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(LIST_HEADING_HEIGHT))
        .justify(Justify::SpaceBetween)
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t("tasks.title")).variant(TypographyStyle::Title))
        .child(pipeline_filter),
    )
    .child(tabs)
    .with_children(rows.iter().map(|row| list_row(ctx, nav, row, now)))
    .child(list_footer(ctx, tasks.tasks.len(), nav))
    .into()
}

/// Task count and the way to the full task list (`WV44u`).
fn list_footer(ctx: &Ctx, count: usize, nav: &ProjectNav) -> Element {
  let nav = nav.clone();
  let count_key = if count == 1 {
    "tasks.count.one"
  } else {
    "tasks.count.other"
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(LIST_FOOTER_HEIGHT))
    .justify(Justify::SpaceBetween)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t_args(count_key, [("count", count.to_string().as_str())])).variant(TypographyStyle::Caption))
    .child(
      pressable(surface().id("view-all-tasks"), move || nav.show(Section::Tasks))
        .hovered_style(Style::new().background(PaletteColor::AccentMuted))
        .child(
          Text::new(&ctx.t("overview.viewAllTasks"))
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextPrimary),
        ),
    )
    .into()
}

/// No tasks yet: what the project is.
fn empty_overview(ctx: &Ctx, open: &OpenProject) -> Element {
  RecoveryPanel::new(
    Icon::FolderGit,
    &ctx.t("overview.emptyTitle"),
    &ctx.t("overview.emptyDetail"),
  )
  .detail(property(&ctx.t("overview.projectKey"), open.project.key().as_str()))
  .detail(property(
    &ctx.t("overview.repository"),
    &display::shorten_path(&open.repo_root, PATH_CHARS),
  ))
  .into()
}
