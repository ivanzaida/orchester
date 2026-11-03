//! Memory, the Agents hub's last tab (`05 Memory` `padSP`): *New record* at the head's right, then the status pills
//! with *Memory settings* at their end, what the open status means and its records, scrolling under the hub's tabs.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::memory::read::ProjectMemory;

use super::{
  Read, record_page,
  state::{MemoryState, Opened},
  status_tab,
  tabs::{self, Order},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::MemoryTab,
  screens::hub::{
    self, Head, HubTab, bones,
    parts::{boxed, group, note},
  },
  theme::{HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    notice::{Notice, NoticeTone},
    skeleton::{self, Skeleton, fading},
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
    Read::Reading | Read::Failed(..) => return reading(ctx, state, view),
    Read::Ready(memory) => memory,
  };
  let Some(view) = view else {
    return reading(ctx, state, view);
  };
  // A record opened from the tab shows as its own page (05c, 05e).
  if let Some(page) = record_page::page(ctx, state, &memory, view.tab) {
    return page;
  }
  let pane = pane
    .child(tabs::strip(
      ctx,
      state.project_id,
      (Some(&memory), view.order),
      view.tab,
    ))
    .child(body(ctx, state, &memory, view));
  hub::list_body_wide(
    "memory-main",
    Column::new().id("memory-body").width(Dimension::Pct(100.0)).child(pane),
    PANE_WIDTH,
  )
}

/// Each waiting proposal's bones while the records are read: its kind, its origin and its text.
const PROPOSALS: [(f32, f32, f32); 3] = [(42.0, 328.0, 412.0), (42.0, 392.0, 378.0), (32.0, 256.0, 230.0)];
/// The proposal's actions while it is read: Approve, Edit and Reject.
const ACTIONS: [f32; 3] = [100.0, 72.0, 88.0];

/// The tab while the records are read (`08 Agents · Memory · loading` `izf4z`): the pills keep their names with bone
/// counts, the note and the *Waiting for your approval* heading stay; each proposal's kind, origin, text and its
/// Approve · Edit · Reject are bones in the card's geometry, the lower ones fading. *Reading memory…*
/// (`memory.reading`) names it.
fn reading(ctx: &mut Ctx, state: &MemoryState, view: Option<View>) -> Element {
  let (order, shown) = view.map_or((Order::of(1), MemoryTab::Waiting), |view| (view.order, view.tab));
  let strip = tabs::strip(ctx, state.project_id, (None, order), shown);
  let cards = PROPOSALS
    .iter()
    .map(|(kind, origin, text)| {
      let actions = Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(ACTIONS.iter().map(|width| skeleton::button(*width)))
        .child(Spacer::new().flex(1.0))
        .child(
          Text::new(&ctx.t("memory.proposal.note"))
            .variant(HubText::Hint.role())
            .nowrap(),
        );
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::Row.role()))
        .spacing(SpacingSize::Md)
        .child(
          Row::new()
            .spacing(SpacingSize::Sm)
            .align_items(Alignment::Center)
            .child(skeleton::line(ctx, HubText::Hint.role(), *kind))
            .child(skeleton::line(ctx, HubText::Hint.role(), *origin)),
        )
        .child(skeleton::line(ctx, TypographyStyle::Body, *text))
        .child(actions)
    })
    .collect();
  let cards: Vec<Element> = fading(cards).into_iter().map(Element::from).collect();
  let queue = group(
    "memory-reading-queue",
    bones::head(
      ctx,
      "memory-reading-queue",
      &ctx.t("memory.tabs.queueTitle"),
      Some(80.0),
    ),
    boxed("memory-reading-queue-rows", cards),
  );
  let pane = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Row.role())
    .child(strip)
    .child(note("memory-tab-note", Icon::Info, &ctx.t("memory.tabs.waitingNote")))
    .child(Skeleton::new("memory-loading", ctx.t("memory.reading")).show(ctx, queue));
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
fn body(ctx: &mut Ctx, state: &MemoryState, memory: &Arc<ProjectMemory>, view: View) -> Element {
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
