//! The Memory tab's status pills (`05` `padSP`): one per status of its records, each with its count (a pill whose
//! count is 0 shows its name only), and Settings at the right end. *Waiting for approval* counts its proposals in
//! `warning` and leads the pills while proposals wait; otherwise it follows *Active*, which the tab then opens on.
//! The order is set once the tab has read its records and never moves under the pointer. A pill is a place of the
//! route (`ProjectPage::Memory`).

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::id::ProjectId;
use orchester_workspace::memory::{listing::StatusFilter, read::ProjectMemory};

use crate::{
  navigation::Navigation,
  routes::{MemoryTab, ProjectPage, Route},
  screens::hub::pills::{self, Pill},
  ui::icon::Icon,
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

  /// The status tabs in order; Settings follows them.
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

/// The statuses a status tab lists; `None` for Settings.
pub const fn statuses(tab: MemoryTab) -> Option<StatusFilter> {
  Some(match tab {
    MemoryTab::Waiting => StatusFilter::Waiting,
    MemoryTab::Active => StatusFilter::Active,
    MemoryTab::Deprecated => StatusFilter::Deprecated,
    MemoryTab::Superseded => StatusFilter::Superseded,
    MemoryTab::Rejected => StatusFilter::RejectedOrWithdrawn,
    MemoryTab::Settings => return None,
  })
}

/// The tab's name: *Waiting for approval*.
pub fn name(ctx: &Ctx, tab: MemoryTab) -> Arc<str> {
  ctx.t(match tab {
    MemoryTab::Waiting => "memory.tabs.waiting",
    MemoryTab::Active => "memory.tabs.active",
    MemoryTab::Deprecated => "memory.tabs.deprecated",
    MemoryTab::Superseded => "memory.tabs.superseded",
    MemoryTab::Rejected => "memory.tabs.rejected",
    MemoryTab::Settings => "memory.tabs.settings",
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
    MemoryTab::Settings => Icon::SlidersHorizontal,
  }
}

/// The route of `tab` of the page.
pub const fn route(project: ProjectId, tab: MemoryTab) -> Route {
  Route::Project(project, ProjectPage::Memory(Some(tab)))
}

/// The status pills in `order` (`05` `padSP`), `shown` the current one, each with its count of `memory` — the
/// proposals that wait in `warning` — and Settings at the right end.
pub fn strip(ctx: &mut Ctx, project: ProjectId, (memory, order): (&ProjectMemory, Order), shown: MemoryTab) -> Element {
  let navigation = Navigation::of(ctx);
  let pill = |ctx: &Ctx, tab: MemoryTab| Pill {
    id: format!("memory-tab-{}", tab.name()).into(),
    label: name(ctx, tab),
    count: statuses(tab).map(|status| (memory.count(status), tab == MemoryTab::Waiting)),
    current: tab == shown,
    open: navigation.to(route(project, tab)),
  };
  let statuses = order.statuses().into_iter().map(|status| pill(ctx, status)).collect();
  let settings = pill(ctx, MemoryTab::Settings);
  pills::strip("memory-tabs", statuses, vec![settings]).into()
}
