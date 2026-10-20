//! The project's Memory page (`Memory · project page` `xY2Uj`): the title with *Memory settings* and *New record*,
//! *Waiting for approval* and *Records* in the main column, the panel at the right. With no record at all it is the
//! empty state (`Empty project memory` `T10K7`); with nothing waiting the queue is one line (`jRGaE`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::memory::{
  listing::{self, Filters},
  read::ProjectMemory,
};

use super::{
  Read,
  editor::opened_on,
  list, opened, panel, proposal,
  state::{MemoryState, Opened},
  toolbar,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, MemoryText, SecretsSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    notice::{Notice, NoticeTone},
    state::{EmptyState, load_error, loading},
  },
};

/// The panel at the right (`PFa0R`).
const PANEL_WIDTH: f32 = 360.0;

pub(super) fn page(ctx: &mut Ctx, state: &MemoryState, read: Read, retries: &Signal<u64>) -> Element {
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
    .child(header(ctx, state));
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
  frame.child(body(ctx, state, &memory)).into()
}

/// *Memory*, what the page is, and its actions (`Xp2gV`).
fn header(ctx: &Ctx, state: &MemoryState) -> Row {
  let open = state.clone();
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .align_items(Alignment::End)
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Snug.role())
        .child(Text::new(&ctx.t("memory.title")).variant(ExtraTypography::Display.role()))
        .child(
          Text::new(&ctx.t_args("memory.description", [("project", &*state.project)])).variant(MemoryText::Lead.role()),
        ),
    )
    .child(
      Row::new().spacing(SpacingSize::Sm).child(settings_button(ctx)).child(
        ActionButton::new("memory-new", ButtonKind::Primary, ctx.t("memory.newRecord"))
          .icon(Icon::Plus)
          .on_click(move || open.open(Opened::New)),
      ),
    )
}

/// *Memory settings*: Project settings › Memory is not built yet, so it says so.
fn settings_button(ctx: &Ctx) -> ActionButton {
  ActionButton::new("memory-settings", ButtonKind::Secondary, ctx.t("memory.settings"))
    .icon(Icon::SlidersHorizontal)
    .unavailable(ctx.t("memory.settingsUnbuilt"))
}

fn body(ctx: &mut Ctx, state: &MemoryState, memory: &Arc<ProjectMemory>) -> Element {
  let empty = memory.records.is_empty() && memory.waiting.is_empty();
  let mut main = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().right(SpacingSize::Sm))
    .spacing(ExtraSpacing::Tabs.role());
  main = main.with_children(list::new_record(ctx, state));
  if memory.unreadable > 0 {
    main = main.child(unreadable(ctx, memory.unreadable));
  }
  main = if empty {
    main.child(empty_state(ctx, state, memory))
  } else {
    main.child(queue(ctx, state, memory)).child(records(ctx, state, memory))
  };
  let side = panel::side(ctx, state, memory);
  Row::new()
    .id("memory-body")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .spacing(SpacingSize::Xl)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .height(Dimension::Pct(100.0))
        .child(
          ScrollVertical::new(main)
            .id("memory-main")
            .width(Dimension::Pct(100.0))
            .height(Dimension::Pct(100.0)),
        ),
    )
    .child(
      Column::new()
        .width(Dimension::Px(PANEL_WIDTH))
        .flex_shrink(0.0)
        .height(Dimension::Pct(100.0))
        .child(side),
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

/// No record at all (`mKozF`): what project memory is, *New record*, *Memory settings*, and how records are saved.
fn empty_state(ctx: &Ctx, state: &MemoryState, memory: &ProjectMemory) -> Element {
  let open = state.clone();
  EmptyState::new(
    "memory-empty",
    Icon::Brain,
    ctx.t("memory.empty.title"),
    ctx.t_args("memory.empty.body", [("project", &*state.project)]),
  )
  .note(panel::saving_summary(ctx, &memory.settings))
  .action(
    ActionButton::new("memory-empty-new", ButtonKind::Primary, ctx.t("memory.newRecord"))
      .icon(Icon::Plus)
      .on_click(move || open.open(Opened::New)),
  )
  .action(settings_button(ctx))
  .into()
}

/// *Waiting for approval* (`QA2RK`): every proposal in one box, each one's review editor in its place; one line
/// that says why records wait here while none does (`MidPY`).
fn queue(ctx: &mut Ctx, state: &MemoryState, memory: &ProjectMemory) -> Element {
  if memory.waiting.is_empty() {
    return Row::new()
      .id("memory-queue-empty")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(icon(Icon::Inbox, IconSize::Md, PaletteColor::TextMuted))
      .child(
        Column::new()
          .flex(1.0)
          .spacing(ExtraSpacing::Tight.role())
          .child(Text::new(&ctx.t("memory.queue.none")).variant(TypographyStyle::Body))
          .child(
            Text::new(&ctx.t_args("memory.queue.noneBody", [("project", &*state.project)]))
              .variant(ExtraTypography::Note.role()),
          ),
      )
      .into();
  }
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Hand, IconSize::Md, PaletteColor::Warning))
    .child(Text::new(&ctx.t("memory.queue.title")).variant(ExtraTypography::FormSection.role()))
    .child(
      Text::new(&memory.waiting.len().to_string())
        .id("memory-queue-count")
        .variant(ExtraTypography::NoteStrong.role())
        .color(PaletteColor::Warning),
    )
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&ctx.t("memory.queue.note")).variant(TypographyStyle::Caption));
  let mut scope: Vec<_> = memory.records.clone();
  scope.extend(memory.replaced.iter().cloned());
  let among = opened::Among {
    records: &scope,
    origins: &memory.origins,
    with_task: true,
    project: Some(memory),
  };
  let mut queue = Column::new()
    .id("memory-queue")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Md)
        .left(ExtraSpacing::Inset.role())
        .right(ExtraSpacing::Inset.role())
        .bottom(ExtraSpacing::Inset.role()),
    )
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(lurq::app::theme::RadiusSize::Lg)
    .child(head);
  for waiting in &memory.waiting {
    let entry = match opened_on(state.opened.get(), waiting) {
      Some(mode) => {
        let props = opened::editor(ctx, waiting, mode, among);
        ctx.mount_keyed::<super::editor::RecordEditor>(
          &format!("memory-editor-{}-{}", waiting.id(), waiting.revision()),
          props,
        )
      }
      None => proposal::proposal(ctx, (waiting, &scope), &memory.origins, proposal::Listed::Queue),
    };
    queue = queue.child(entry);
  }
  queue.into()
}

/// *Records*: the count and the order, the search and filters, and the grouped list (`lyOr9`).
fn records(ctx: &mut Ctx, state: &MemoryState, memory: &ProjectMemory) -> Element {
  let filters = state.filters.get();
  let shown: Vec<_> = memory
    .records
    .iter()
    .filter(|record| record.status() != orchester_core::memory::MemoryStatus::Proposed)
    .cloned()
    .collect();
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("memory.records")).variant(TypographyStyle::Title))
    .child(
      Text::new(&count(ctx, &shown, &filters, memory.unreadable))
        .id("memory-records-count")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&ctx.t("memory.order")).variant(TypographyStyle::Caption));
  let bar = ctx.mount_keyed::<toolbar::FilterBar>(
    "memory-project-filters",
    toolbar::FiltersProps {
      project: true,
      tags: listing::tags(&shown).into(),
    },
  );
  let input = list::ListInput {
    records: &shown,
    origins: &memory.origins,
    filters: &filters,
    task_tab: false,
  };
  let groups = list::groups(ctx, state, input);
  let mut list = Column::new().id("memory-records").width(Dimension::Pct(100.0));
  list = list.with_children(list::row_failure(ctx, state, &shown));
  list = if groups.is_empty() {
    list.child(super::no_match(ctx, state, &shown, &filters))
  } else {
    list.with_children(groups)
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(head)
    .child(bar)
    .child(list)
    .into()
}

/// How many records the status filter shows (*10 active*, *2 deprecated*), and how many were left out unread.
fn count(ctx: &Ctx, records: &[orchester_core::memory::Memory], filters: &Filters, unreadable: usize) -> Arc<str> {
  let shown = listing::of_status(records, filters.status);
  let counted = ctx.t_args(
    "memory.recordsCount",
    [
      ("count", shown.to_string()),
      (
        "status",
        super::words::status_filter(ctx, filters.status).to_lowercase(),
      ),
    ],
  );
  if unreadable == 0 {
    return counted;
  }
  ctx.t_args(
    "memory.recordsLeftOut",
    [("count", counted.to_string()), ("unreadable", unreadable.to_string())],
  )
}
