//! The project's Memory page (`Content · Memory` `ketjQ` in `CIThk`): the title with *New record*, the tabs under it
//! and the open tab's body, which scrolls. Nothing sits beside the body, so nothing runs past the window's right edge
//! at any width the window takes (`ghfRl` at 960).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::memory::read::ProjectMemory;

use super::{
  Read,
  settings::{SettingsProps, SettingsTab},
  state::{MemoryState, Opened},
  status_tab,
  tabs::{self, Order},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::MemoryTab,
  theme::{ExtraSpacing, ExtraTypography, MemoryText, SecretsSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    state::{load_error, loading},
  },
};

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
  let frame = Column::new()
    .id("screen-memory")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Roomy.role())
        .bottom(SpacingSize::Xl)
        .left(SecretsSpacing::Page.role())
        .right(SecretsSpacing::Page.role()),
    )
    .spacing(SpacingSize::Lg)
    .child(header(ctx, state, view.map(|view| view.tab)));
  let memory = match read {
    Read::Reading => return frame.child(loading("memory-loading", &ctx.t("memory.reading"))).into(),
    Read::Failed(reason) => {
      let retries = retries.clone();
      let body = ctx.t_args("memory.loadFailedBody", [("reason", message_text(ctx, &reason))]);
      return frame
        .child(load_error(
          ctx,
          "memory-load-error",
          (&ctx.t("memory.loadFailed"), &body),
          move || retries.update(|count| *count += 1),
        ))
        .into();
    }
    Read::Ready(memory) => memory,
  };
  let Some(view) = view else {
    return frame.child(loading("memory-loading", &ctx.t("memory.reading"))).into();
  };
  frame
    .child(tabs::strip(ctx, state.project_id, (&memory, view.order), view.tab))
    .child(body(ctx, state, (project, &memory), view))
    .into()
}

/// *Memory*, what the page is, and *New record*, which opens its editor on the Active tab (`f3mwij`).
fn header(ctx: &mut Ctx, state: &MemoryState, shown: Option<MemoryTab>) -> Row {
  let (open, navigation, project) = (state.clone(), Navigation::of(ctx), state.project_id);
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::End)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Snug.role())
        .child(Text::new(&ctx.t("memory.title")).variant(ExtraTypography::Display.role()))
        .child(
          Text::new(&ctx.t_args("memory.description", [("project", &*state.project)])).variant(MemoryText::Lead.role()),
        ),
    )
    .child(
      ActionButton::new("memory-new", ButtonKind::Primary, ctx.t("memory.newRecord"))
        .icon(Icon::Plus)
        .on_click(move || {
          open.open(Opened::New);
          if shown != Some(MemoryTab::Active) {
            navigation.go(tabs::route(project, MemoryTab::Active));
          }
        }),
    )
}

/// The open tab's body, scrolling under the tabs.
fn body(
  ctx: &mut Ctx,
  state: &MemoryState,
  (project, memory): (&ShownProject, &Arc<ProjectMemory>),
  view: View,
) -> Element {
  let content: Element = if view.tab == MemoryTab::Settings {
    ctx.mount_keyed::<SettingsTab>(
      "memory-settings-tab",
      SettingsProps {
        project: project.clone(),
        stored: memory.settings,
      },
    )
  } else {
    let shown = status_tab::Shown {
      memory,
      tab: view.tab,
      order: view.order,
    };
    status_tab::body(ctx, state, shown).into()
  };
  Column::new()
    .id("memory-body")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .child(
      ScrollVertical::new(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding(Padding::new().right(SpacingSize::Sm))
          .child(content),
      )
      .id("memory-main")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0)),
    )
    .into()
}

/// *N records couldn't be read and are left out* (`GCHz6`).
pub(crate) fn unreadable(ctx: &Ctx, count: usize) -> Element {
  let title = if count == 1 {
    ctx.t("memory.unreadable.one")
  } else {
    ctx.t_args("memory.unreadable.other", [("count", count.to_string())])
  };
  Notice::new("memory-unreadable", NoticeTone::Danger, title)
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
