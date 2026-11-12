//! The panel's head (`Head` `E6whl0`, `Chips` `z6peUe` and `Notice · replaces` `cNzwQ` in Y1 `tsfxN`): *Memory* and
//! the record's status over its title, its chips with who wrote it at the row's end, and what approving it replaces
//! or what deprecating it did.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::memory::{Memory, MemoryAuthor, MemoryStatus};
use orchester_workspace::memory::read::ProjectMemory;

use crate::{
  screens::{memory::words, team_hub::panel::head_named},
  theme::{ExtraSpacing, HubText, IconSize, MemoryText},
  ui::{
    chip::{BadgeTone, TagChip, badge_in},
    icon::{Icon, icon},
  },
};

/// *Memory* and the record's status over its title (`Head` `E6whl0`): *Review proposed decision*, *Decision ·
/// webhook-retry*, *Warning*.
pub(super) fn head(ctx: &Ctx, record: &Memory) -> Row {
  let kind = record.content().kind();
  let title = if record.status() == MemoryStatus::Proposed {
    ctx.t_args("memory.editor.reviewTitle", [("kind", &*words::kind_lower(ctx, kind))])
  } else {
    match record.content().key() {
      Some(key) => ctx.t_args(
        "memory.page.titleKeyed",
        [
          ("kind", words::kind_name(ctx, kind).to_string()),
          ("key", key.as_str().to_owned()),
        ],
      ),
      None => words::kind_name(ctx, kind),
    }
  };
  let status = status_word(ctx, record.status());
  head_named(
    &title,
    (ctx.t("memory.title").to_string(), Some(status.to_string())),
    None,
  )
}

/// The record's status as the panel's head says it: *Waiting for approval*, *Active*.
pub(in crate::screens::memory) fn status_word(ctx: &Ctx, status: MemoryStatus) -> Arc<str> {
  ctx.t(match status {
    MemoryStatus::Proposed => "memory.tabs.waiting",
    MemoryStatus::Active => "memory.status.active",
    MemoryStatus::Deprecated => "memory.status.deprecated",
    MemoryStatus::Superseded => "memory.status.superseded",
    MemoryStatus::Rejected => "memory.status.rejected",
    MemoryStatus::Withdrawn => "memory.status.withdrawn",
  })
}

/// The chips (`Chips` `z6peUe`): the status's badge, the type, whose record it is, *Pinned* and *This computer*; at
/// the row's end, right-aligned to the panel's edge, who wrote it and what happened to it since, cut with an ellipsis
/// when it runs out of room. The chips wrap onto another line before they push it out.
pub(super) fn chips(ctx: &Ctx, memory: &ProjectMemory, record: &Memory, team: bool) -> Row {
  let (tone, word) = match record.status() {
    MemoryStatus::Proposed => (BadgeTone::Warning, ctx.t("memory.proposal.waiting")),
    MemoryStatus::Active => (BadgeTone::Success, ctx.t("memory.status.active")),
    MemoryStatus::Deprecated => (BadgeTone::Danger, ctx.t("memory.status.deprecated")),
    status => (BadgeTone::Neutral, status_word(ctx, status)),
  };
  let kind = record.content().kind();
  let chips = Row::new()
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0))
    .wrap()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(badge_in("memory-panel-status", tone, &word, MemoryText::Chip.role()))
    .child(TagChip::new("memory-panel-kind", words::kind_name(ctx, kind)).glyph(words::glyph(kind)))
    .child(TagChip::new(
      "memory-panel-scope",
      words::scope(ctx, record, &memory.origins),
    ))
    .with_children(
      record
        .is_pinned()
        .then(|| TagChip::new("memory-panel-pinned", ctx.t("memory.hub.pinned")).glyph(Icon::Pin)),
    )
    .with_children(
      team.then(|| TagChip::new("memory-panel-computer", ctx.t("memory.hub.thisComputer")).glyph(Icon::Laptop)),
    );
  Row::new()
    .id("memory-panel-chips")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(chips)
    .child(Row::new().flex(1.0).min_width(Dimension::Px(0.0)))
    .child(
      Text::new(&written(ctx, memory, record))
        .id("memory-panel-written")
        .variant(HubText::PanelHint.role())
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
}

/// Who wrote the record and when, or what happened to it since: *proposed by Builder · Nov 10, 16:01*, *by Builder ·
/// edited by you Nov 8*, *by you · Nov 2*.
fn written(ctx: &Ctx, memory: &ProjectMemory, record: &Memory) -> Arc<str> {
  let agent = memory.origins.written(record).and_then(|place| place.agent);
  let mut parts = vec![match (&agent, record.status()) {
    (Some(agent), MemoryStatus::Proposed) => ctx
      .t_args("memory.editor.proposedBy", [("agent", &**agent)])
      .to_string(),
    (Some(agent), _) => ctx.t_args("memory.hub.by", [("agent", &**agent)]).to_string(),
    (None, _) if matches!(record.author(), MemoryAuthor::Agent(_)) => ctx.t("memory.hub.byAgent").to_string(),
    (None, _) => ctx.t("memory.hub.byYou").to_string(),
  }];
  let since = words::change(ctx, record);
  if since.is_empty() {
    parts.push(words::when(ctx, record.created_at()));
  } else {
    parts.extend(since);
  }
  words::join(ctx, &parts)
}

/// What approving a proposal replaces (`Notice · replaces` `cNzwQ` in Y1), or what deprecating a record did (`Notice`
/// `igQyv` in Y6); none on the others.
pub(super) fn notice(ctx: &Ctx, memory: &ProjectMemory, record: &Memory) -> Option<Element> {
  let (glyph, mark, title, body) = match record.status() {
    MemoryStatus::Proposed => {
      let replaced = memory
        .records
        .iter()
        .chain(&memory.replaced)
        .find(|old| Some(old.id()) == record.supersedes() && old.status() == MemoryStatus::Active)?;
      let kind = words::kind_lower(ctx, replaced.content().kind());
      let title = match replaced.content().key() {
        Some(key) => ctx.t_args("memory.editor.replacesKeyed", [("kind", &*kind), ("key", key.as_str())]),
        None => ctx.t_args("memory.editor.replaces", [("kind", &*kind)]),
      };
      (
        Icon::Replace,
        PaletteColor::Info,
        title,
        ctx.t("memory.editor.replacesTextDiff"),
      )
    }
    MemoryStatus::Deprecated => (
      Icon::Archive,
      PaletteColor::TextMuted,
      ctx.t("memory.hub.deprecatedTitle"),
      ctx.t("memory.hub.deprecatedBody"),
    ),
    _ => return None,
  };
  Some(boxed_notice((glyph, mark), &title, &body))
}

/// A notice in the panel (`Notice · replaces` `cNzwQ`): on `surface_raised` in a `border` stroke, radius `md`, 12 at
/// its sides and 10 over and under; a 14 glyph in `mark`, 10 before the title in 12.5 `text_primary` over the body in
/// 12/1.45 `text_muted`, 4 apart.
fn boxed_notice((glyph, mark): (Icon, PaletteColor), title: &str, body: &str) -> Element {
  let id = "memory-panel-notice";
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(
      Row::new()
        .padding(Padding::new().top(ExtraSpacing::Tight.role()))
        .child(icon(glyph, IconSize::Md, mark)),
    )
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Xs)
        .child(
          Text::new(title)
            .id(format!("{id}-title"))
            .variant(HubText::NoticeTitle.role()),
        )
        .child(
          Text::new(body)
            .id(format!("{id}-body"))
            .variant(HubText::ListNote.role()),
        ),
    )
    .into()
}
