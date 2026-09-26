//! The overview's task list (`BPSF0`): the newest tasks of the chosen tab — all, running, needing
//! the user, queued — with each tab's count (`i2uFe`), and how many tasks are active and
//! completed (`WV44u`). The pipeline filter is not available yet.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, dimension::Dimension},
};
use orchester_core::time::Timestamp;

use super::super::{ProjectNav, Section, task_rows::list_row};
use crate::{
  tasks::{
    data::ProjectTasks,
    overview::{OverviewTab, footer},
  },
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{pressable, surface},
    tab::{Tab, tab_row},
  },
};

/// Task list heading, tab row, tab and footer heights (`aZa78`, `i2uFe`, `SOW6S`, `WV44u`).
const LIST_HEADING_HEIGHT: f32 = 34.0;
const TAB_ROW_HEIGHT: f32 = 48.0;
const TAB_HEIGHT: f32 = 38.0;
const LIST_FOOTER_HEIGHT: f32 = 54.0;
/// Tasks shown in the overview; the task list has all of them.
const RECENT_TASKS: usize = 6;

pub(super) fn task_list(ctx: &Ctx, tasks: &ProjectTasks, tab: &Signal<OverviewTab>, nav: &ProjectNav) -> Element {
  let now = Timestamp::now();
  let selected = tab.get();
  let rows = selected.rows(&tasks.tasks, &tasks.pipelines, &tasks.phases, RECENT_TASKS);
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
    OverviewTab::ALL.into_iter().map(|each| {
      let signal = tab.clone();
      Tab::new(tab_id(each), ctx.t(each.label_key()), Dimension::Px(TAB_HEIGHT))
        .count(Some(each.count(&tasks.tasks, &tasks.phases)))
        .selected(each == selected)
        .on_click(move || signal.set(each))
    }),
  );
  let list: Vec<Element> = if rows.is_empty() {
    vec![
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding_vertical(SpacingSize::Md)
        .child(Text::new(&ctx.t("tasks.emptyTab")).variant(TypographyStyle::Caption))
        .into(),
    ]
  } else {
    rows.iter().map(|row| list_row(ctx, nav, row, now)).collect()
  };
  Column::new()
    .id("overview-tasks")
    .flex(1.0)
    .height(Dimension::Pct(100.0))
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
    .with_children(list)
    .child(list_footer(ctx, tasks, nav))
    .into()
}

const fn tab_id(tab: OverviewTab) -> &'static str {
  match tab {
    OverviewTab::All => "overview-tab-all",
    OverviewTab::Running => "overview-tab-running",
    OverviewTab::NeedsYou => "overview-tab-needs-you",
    OverviewTab::Queued => "overview-tab-queued",
  }
}

/// Active and completed tasks, and the way to the full task list (`WV44u`).
fn list_footer(ctx: &Ctx, tasks: &ProjectTasks, nav: &ProjectNav) -> Element {
  let nav = nav.clone();
  Row::new()
    .id("overview-footer")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(LIST_FOOTER_HEIGHT))
    .justify(Justify::SpaceBetween)
    .align_items(Alignment::Center)
    .child(Text::new(&message_text(ctx, &footer(&tasks.tasks, &tasks.phases))).variant(TypographyStyle::Caption))
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
