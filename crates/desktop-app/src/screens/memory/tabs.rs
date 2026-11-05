//! The Memory tab's status pills (`05` `padSP`): one per status of its records, each with its count (a pill whose
//! count is 0 shows its name only), and *Memory settings* at the right end, which opens Project settings › Context
//! (`S15` `BJQn0`). *Waiting for approval* counts its proposals in `warning` and leads the pills while proposals wait;
//! otherwise it follows *Active*, which the tab then opens on. The order is set once the tab has read its records and
//! never moves under the pointer. A pill is a place of the route (`ProjectPage::Memory`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::Text,
  layout::Alignment,
  node::Element,
};
use orchester_core::id::ProjectId;
use orchester_workspace::memory::{listing::StatusFilter, read::ProjectMemory};

use crate::{
  navigation::{Navigation, link::Open},
  routes::{MemoryTab, ProjectPage, ProjectSettingsTab, Route},
  screens::hub::pills::{self, Pill},
  theme::{ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::opening,
  },
};

/// The order of the status tabs, set when the page opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Order {
  waiting_first: bool,
}

impl Order {
  /// The order for a page that opens while `waiting` proposals wait.
  pub const fn of(waiting: usize) -> Self {
    Self {
      waiting_first: waiting > 0,
    }
  }

  /// The tab the page opens on when the route names none.
  pub const fn opening(self) -> MemoryTab {
    if self.waiting_first {
      MemoryTab::Waiting
    } else {
      MemoryTab::Active
    }
  }

  /// The status tabs in order.
  pub const fn statuses(self) -> [MemoryTab; 5] {
    let (first, second) = if self.waiting_first {
      (MemoryTab::Waiting, MemoryTab::Active)
    } else {
      (MemoryTab::Active, MemoryTab::Waiting)
    };
    [
      first,
      second,
      MemoryTab::Deprecated,
      MemoryTab::Superseded,
      MemoryTab::Rejected,
    ]
  }
}

/// The statuses a status tab lists.
pub const fn statuses(tab: MemoryTab) -> StatusFilter {
  match tab {
    MemoryTab::Waiting => StatusFilter::Waiting,
    MemoryTab::Active => StatusFilter::Active,
    MemoryTab::Deprecated => StatusFilter::Deprecated,
    MemoryTab::Superseded => StatusFilter::Superseded,
    MemoryTab::Rejected => StatusFilter::RejectedOrWithdrawn,
  }
}

/// The tab's name: *Waiting for approval*.
pub fn name(ctx: &Ctx, tab: MemoryTab) -> Arc<str> {
  ctx.t(match tab {
    MemoryTab::Waiting => "memory.tabs.waiting",
    MemoryTab::Active => "memory.tabs.active",
    MemoryTab::Deprecated => "memory.tabs.deprecated",
    MemoryTab::Superseded => "memory.tabs.superseded",
    MemoryTab::Rejected => "memory.tabs.rejected",
  })
}

/// The glyph of a status tab, on its empty state and on the button that opens it.
pub const fn glyph(tab: MemoryTab) -> Icon {
  match tab {
    MemoryTab::Waiting => Icon::Inbox,
    MemoryTab::Active => Icon::CircleCheck,
    MemoryTab::Deprecated => Icon::Archive,
    MemoryTab::Superseded => Icon::Replace,
    MemoryTab::Rejected => Icon::CircleX,
  }
}

/// The route of `tab` of the page.
pub const fn route(project: ProjectId, tab: MemoryTab) -> Route {
  Route::Project(project, ProjectPage::Memory(Some(tab)))
}

/// The status pills in `order` (`05` `padSP`), `shown` the current one, each with its count of `memory` — the
/// proposals that wait in `warning` — and *Memory settings* at the right end; while the records are read (`memory`
/// none), each status's count is a bone.
pub fn strip(
  ctx: &mut Ctx,
  project: ProjectId,
  (memory, order): (Option<&ProjectMemory>, Order),
  shown: MemoryTab,
) -> Element {
  let navigation = Navigation::of(ctx);
  let pill = |ctx: &Ctx, tab: MemoryTab| Pill {
    id: format!("memory-tab-{}", tab.name()).into(),
    label: name(ctx, tab),
    count: memory.map(|memory| (memory.count(statuses(tab)), tab == MemoryTab::Waiting)),
    reading: memory.is_none(),
    current: tab == shown,
    open: navigation.to(route(project, tab)),
  };
  let statuses = order.statuses().into_iter().map(|status| pill(ctx, status)).collect();
  let context = navigation.to(Route::Project(
    project,
    ProjectPage::Settings(ProjectSettingsTab::Context),
  ));
  pills::strip("memory-tabs", statuses, settings_link(ctx, context)).into()
}

/// *Memory settings ↗* (`Settings link` `m24yD`): a 13 `text_muted` `settings-2`, the words in 12 `text_secondary` and
/// a 13 `text_muted` arrow, 5 apart. It opens Project settings › Context, which holds the Memory group.
fn settings_link(ctx: &Ctx, context: Open) -> Element {
  let label = ctx.t("memory.tabs.settingsLink");
  opening(context)
    .id("memory-settings-link")
    .describe("a11y_name", label.clone())
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Settings2, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&label)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(icon(Icon::ArrowUpRight, IconSize::Base, PaletteColor::TextMuted))
    .into()
}
