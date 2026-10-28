//! Memory, the Agents hub's last tab (`05 Memory` `padSP`): *New record* at the head's right, then the status pills,
//! what the open status means and its records, or the tab's settings, scrolling under the hub's tabs.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::memory::read::ProjectMemory;

use super::{
  Read, record_page,
  settings::{SettingsProps, SettingsTab},
  state::{MemoryState, Opened},
  status_tab,
  tabs::{self, Order},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::MemoryTab,
  screens::hub::{self, Head, HubTab},
  theme::HubSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    notice::{Notice, NoticeTone},
    state::loading,
  },
};

/// The page's width (`Pane` `Jy4nw` in 05), wider than the hub's other lists.
const PANE_WIDTH: f32 = 760.0;

/// The tab the page shows, among its tabs in the order they were set in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct View {
  pub order: Order,
  pub tab: MemoryTab,
}

pub(super) fn page(
  ctx: &mut Ctx,
  state: &MemoryState,
  (project, read, retries): (&ShownProject, Read, &Signal<u64>),
  view: Option<View>,
) -> Element {
  let head = Head::actions([new_record(ctx, state, view.map(|view| view.tab))]);
  let content = content(ctx, state, (project, read, retries), view);
  Column::new()
    .id("screen-memory")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(hub::page(ctx, project, (HubTab::Memory, head), content))
    .into()
}

fn content(
  ctx: &mut Ctx,
  state: &MemoryState,
  (project, read, retries): (&ShownProject, Read, &Signal<u64>),
  view: Option<View>,
) -> Element {
  let pane = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Row.role());
  let failed = match &read {
    Read::Failed(reason, retrying) => Some((reason.clone(), *retrying)),
    Read::Reading | Read::Ready(_) => None,
  };
  let again = retries.clone();
  let what = ctx.t("memory.loadFailed");
  let failure = hub::read_failure(
    ctx,
    project,
    (HubTab::Memory, "memory-load-error", &what),
    failed,
    move || again.update(|count| *count += 1),
  );
  if let Some(failure) = failure {
    let failure = failure.still_works(ctx.t("memory.loadStillWorks"));
    return hub::list_body("memory-failed", failure.page(ctx));
  }
  let memory = match read {
    Read::Reading | Read::Failed(..) => return loading("memory-loading", &ctx.t("memory.reading")).into(),
    Read::Ready(memory) => memory,
  };
  let Some(view) = view else {
    return loading("memory-loading", &ctx.t("memory.reading")).into();
  };
  // A record opened from the tab shows as its own page (05c, 05e).
  if let Some(page) = record_page::page(ctx, state, &memory, view.tab) {
    return page;
  }
  let pane = pane
    .child(tabs::strip(ctx, state.project_id, (&memory, view.order), view.tab))
    .child(body(ctx, state, (project, &memory), view));
  hub::list_body_wide(
    "memory-main",
    Column::new().id("memory-body").width(Dimension::Pct(100.0)).child(pane),
    PANE_WIDTH,
  )
}

/// *New record*, which opens its editor on the Active tab (`f3mwij`).
fn new_record(ctx: &mut Ctx, state: &MemoryState, shown: Option<MemoryTab>) -> Element {
  let (open, navigation, project) = (state.clone(), Navigation::of(ctx), state.project_id);
  ActionButton::new("memory-new", ButtonKind::Primary, ctx.t("memory.newRecord"))
    .icon(Icon::Plus)
    .on_click(move || {
      open.open(Opened::New);
      if shown != Some(MemoryTab::Active) {
        navigation.go(tabs::route(project, MemoryTab::Active));
      }
    })
    .into()
}

/// The open tab's body under the pills.
fn body(
  ctx: &mut Ctx,
  state: &MemoryState,
  (project, memory): (&ShownProject, &Arc<ProjectMemory>),
  view: View,
) -> Element {
  if view.tab == MemoryTab::Settings {
    return ctx.mount_keyed::<SettingsTab>(
      "memory-settings-tab",
      SettingsProps {
        project: project.clone(),
        stored: memory.settings,
      },
    );
  }
  let shown = status_tab::Shown {
    memory,
    tab: view.tab,
    order: view.order,
  };
  status_tab::body(ctx, state, shown).into()
}

/// *N records couldn't be read and are left out* (`GCHz6`).
pub(crate) fn unreadable(ctx: &Ctx, count: usize) -> Element {
  let title = if count == 1 {
    ctx.t("memory.unreadable.one")
  } else {
    ctx.t_args("memory.unreadable.other", [("count", count.to_string())])
  };
  // What was read stays: information with the `danger` icon, not a red fill (`SsC16`).
  Notice::new("memory-unreadable", NoticeTone::Info, title)
    .icon(Icon::CircleAlert)
    .mark(lurq::app::theme::PaletteColor::Danger)
    .body(ctx.t("memory.unreadable.body"))
    .into()
}

/// The page's tabs in `order` showing the route's `tab`, else the one the page opens on.
pub(super) fn view(order: Order, tab: Option<MemoryTab>) -> View {
  View {
    order,
    tab: tab.unwrap_or(order.opening()),
  }
}
