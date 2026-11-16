//! The Memory filter's sub-filter (`Memory filter` `cg7EL` in Y1 `tsfxN`): a pill per status in its order, each with
//! its count (one whose count is 0 shows its name only, *Waiting for approval* counts in `warning`), then at the line's
//! end the search of the records' text, keys and tags, 260 wide, and *Memory settings ↗*, which opens Project settings
//! › Context (`S15` `BJQn0`). In a team project a line under it says this computer's memory is not shared with the team
//! (`Scope line · this computer` `klW3k` in Y5 `Fnu7A`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::memory::read::ProjectMemory;

use super::pills::Pill;
use crate::{
  navigation::Navigation,
  routes::{MemoryTab, ProjectPage, ProjectSettingsTab, Route},
  screens::memory::{
    state::MemoryState,
    tabs::{self, Order},
  },
  theme::{ExtraSpacing, HubText, IconSize},
  ui::{
    field::FieldInput,
    icon::{Icon, icon},
    pressable::opening,
  },
};

/// The search's width (`Search` `AhQeG`).
const SEARCH_WIDTH: f32 = 260.0;

/// The pills in `order`, `shown` the current one, each with its count of `memory`; while the records are read
/// (`memory` none) each count is a bone.
pub(super) fn sub_filter(
  ctx: &mut Ctx,
  state: &MemoryState,
  (memory, order): (Option<&ProjectMemory>, Order),
  shown: MemoryTab,
) -> Element {
  let navigation = Navigation::of(ctx);
  let project = state.project_id;
  let pills: Vec<Element> = order
    .statuses()
    .into_iter()
    .map(|tab| {
      Element::from(Pill {
        id: format!("memory-tab-{}", tab.name()).into(),
        label: tabs::name(ctx, tab),
        count: memory.map(|memory| (memory.count(tabs::statuses(tab)), tab == MemoryTab::Waiting)),
        reading: memory.is_none(),
        current: tab == shown,
        open: navigation.to(tabs::route(project, tab)),
      })
    })
    .collect();
  let search = FieldInput::new("memory-search", ctx.t("memory.toolbar.search"), state.search.clone())
    .search()
    .placeholder(ctx.t("memory.toolbar.search"))
    .clearable(ctx.t("memory.toolbar.clearSearch"))
    .width(Dimension::Pct(100.0))
    .render(ctx);
  // The search gives up its width first when the window is narrow, so the line never runs past it.
  let search = Row::new()
    .width(Dimension::Px(SEARCH_WIDTH))
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0))
    .child(search);
  Row::new()
    .id("memory-tabs")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .with_children(pills)
    .child(Row::new().flex(1.0))
    .child(search)
    .child(settings_link(ctx, project))
    .into()
}

/// *Memory settings ↗* (`Memory settings` `h1W9Ro`): the words in 12 `text_secondary` and a 13 `text_muted` arrow, 4
/// apart, 8 from the search. It opens Project settings › Context, which holds the Memory group.
fn settings_link(ctx: &mut Ctx, project: ProjectId) -> Element {
  let context = Navigation::of(ctx).to(Route::Project(
    project,
    ProjectPage::Settings(ProjectSettingsTab::Context),
  ));
  let label = ctx.t("memory.tabs.settingsLink");
  opening(context)
    .id("memory-settings-link")
    .describe("a11y_name", label.clone())
    .padding(Padding::new().left(SpacingSize::Sm))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(Text::new(&label).variant(HubText::Status.role()).nowrap())
    .child(icon(Icon::ArrowUpRight, IconSize::Base, PaletteColor::TextMuted))
    .into()
}

/// *This computer's memory of Kontur · not shared with the team: …* (`klW3k`): a 13 `laptop` and the line in 12
/// `text_muted`, 8 apart, 2 in at its sides.
pub(super) fn not_shared(ctx: &Ctx, project: &str) -> Element {
  Row::new()
    .id("memory-not-shared")
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(ExtraSpacing::Tight.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Laptop, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t_args("memory.hub.notShared", [("project", project)]))
        .id("memory-not-shared-text")
        .variant(HubText::PanelHint.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}
