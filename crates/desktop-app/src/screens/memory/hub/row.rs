//! A record in the Memory filter's list (`Item · webhook-retry proposal` `PdesS` in Y1, `PlgOF` in Y2), a row of the
//! hub's list: its type's 16 glyph in `text_muted`; what it says on one line (a handoff's summary) over its meta line —
//! whose it is, its key, who wrote it where and when, what happened to it since — both cut with an ellipsis before
//! its end; at its end the pin of a pinned record, the task's latest handoff's *Latest*, its first tag and how many
//! more it has. A record no agent is given says what it says in `text_muted` (Y6 `JLsNL`). A press opens it in the
//! panel, closing what was open in place of another record's.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, Text},
  layout::Alignment,
  node::Element,
};
use orchester_core::memory::{Memory, MemoryKind, MemoryStatus};
use orchester_workspace::memory::{handoff::HandoffParts, origin::Origins};

use crate::{
  navigation::Navigation,
  routes::{MemoryTab, MemoryView, ProjectPage, Route},
  screens::{
    hub::agent_row::{self, AgentRow, agent_row, tag_chip},
    memory::{state::MemoryState, words},
  },
  theme::{ExtraSpacing, HubText, IconSize},
  ui::{
    chip::{BadgeTone, badge},
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// What a row shows besides its record.
pub(super) struct RowOf<'a> {
  pub tab: MemoryTab,
  pub chosen: bool,
  /// The task's latest handoff, which every agent of the task is given.
  pub latest: bool,
  pub origins: &'a Origins,
}

pub(super) fn row(ctx: &mut Ctx, state: &MemoryState, memory: &Memory, of: &RowOf<'_>) -> Element {
  let id = format!("memory-record-{}", memory.id());
  let given = matches!(memory.status(), MemoryStatus::Active | MemoryStatus::Proposed);
  let route = Route::Project(
    state.project_id,
    ProjectPage::Memory(MemoryView::new(Some(of.tab), Some(memory.id()))),
  );
  let (navigation, close) = (Navigation::of(ctx), state.clone());
  let open = pressable(move || {
    close.close();
    navigation.go(route.clone());
  });
  let line = line(ctx, memory, of.origins);
  agent_row(AgentRow {
    id: &id,
    chosen: of.chosen,
    open,
    group: None,
    kind: memory.content().kind().name().to_owned(),
    mark: icon(
      words::glyph(memory.content().kind()),
      IconSize::Lg,
      PaletteColor::TextMuted,
    )
    .into(),
    name: (
      &said(memory),
      if given {
        PaletteColor::TextPrimary
      } else {
        PaletteColor::TextMuted
      },
    ),
    line: agent_row::line(&id, &line, PaletteColor::TextMuted).into(),
    tag: Some(end(ctx, memory, of.latest, &id)),
    version: None,
  })
}

/// What a record says on its one line: a handoff's summary, else its text's first line.
fn said(memory: &Memory) -> String {
  let content = memory.content();
  let handoff = (content.kind() == MemoryKind::Handoff).then(|| HandoffParts::of(content.text()));
  handoff
    .as_ref()
    .map_or(content.text(), |parts| parts.summary.as_str())
    .lines()
    .next()
    .unwrap_or_default()
    .to_owned()
}

/// Whose record it is, its key, then who wrote it where and when: a proposal with the agent's confidence, a record
/// with what happened to it since.
fn line(ctx: &Ctx, memory: &Memory, origins: &Origins) -> String {
  let mut parts = vec![words::scope(ctx, memory, origins).to_string()];
  parts.extend(
    memory
      .content()
      .key()
      .map(|key| ctx.t_args("memory.row.key", [("key", key.as_str())]).to_string()),
  );
  let meta = if memory.status() == MemoryStatus::Proposed {
    words::proposal_meta(ctx, memory, origins, false)
  } else {
    words::meta(ctx, memory, origins, false)
  };
  parts.push(meta.to_string());
  words::join(ctx, &parts).to_string()
}

/// The row's end (`WlEYH`, `ZIPoE`, `zNVxq`, `f2bFf` in Y1 and Y2): the pin, *Latest*, the first tag and *+N*.
fn end(ctx: &Ctx, memory: &Memory, latest: bool, id: &str) -> Row {
  let tags = memory.content().tags();
  let more = tags.iter().count().saturating_sub(1);
  Row::new()
    .id(format!("{id}-end"))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .with_children(
      memory
        .is_pinned()
        .then(|| icon(Icon::Pin, IconSize::Md, PaletteColor::TextPrimary).id(format!("{id}-pinned"))),
    )
    .with_children(latest.then(|| badge(&format!("{id}-latest"), BadgeTone::Info, &ctx.t("memory.row.latest"))))
    .with_children(tags.iter().next().map(|tag| tag_chip(&format!("{id}-tag"), tag)))
    .with_children((more > 0).then(|| {
      Text::new(&ctx.t_args("memory.hub.moreTags", [("count", more.to_string())]))
        .id(format!("{id}-more-tags"))
        .variant(HubText::RowLine.role())
        .color(PaletteColor::TextMuted)
        .nowrap()
    }))
}
