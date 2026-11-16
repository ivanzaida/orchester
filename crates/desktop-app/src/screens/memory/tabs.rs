//! The statuses of the Memory filter's sub-filter (`Memory filter` `cg7EL` in Y1 `tsfxN`): their order, names and
//! glyphs, the records each lists and its route. *Waiting for approval* leads the statuses while proposals wait;
//! otherwise it follows *Active*, which the filter then opens on (Y4 `i7aEec`). The order is set once the filter has
//! read its records and never moves under the pointer.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::id::ProjectId;
use orchester_workspace::memory::listing::StatusFilter;

use crate::{
  routes::{MemoryTab, MemoryView, ProjectPage, Route},
  ui::icon::Icon,
};

/// The order of the statuses, set when the filter opens.
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

/// The route of `tab`, with the first record of its list.
pub const fn route(project: ProjectId, tab: MemoryTab) -> Route {
  Route::Project(project, ProjectPage::Memory(MemoryView::on(tab)))
}
