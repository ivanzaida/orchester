//! An agent's tool groups (`Tool group` `u0aS4V`): one chip per group at the chat's edge, 26 high on
//! `surface_input` in a `border` stroke, radius 6, 8 at its sides: a 12 `wrench` (a turning loader while its
//! command runs), what the group did in 11.5 `text_secondary` (*Read 4 files · searched 2 times · ran cargo
//! test*), a failed command's exit in `danger` (*exit 1*), and a chevron, 6 apart. Pressed, it opens the
//! group's rows under it, 20 in (`Tool rows` `G8Q14`, [`super::entry`]): the first seven, then *Show N more ·
//! 4 reads, 1 search*.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Text, TextOverflow},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_workspace::tasks::{
  timeline::{
    ToolGroup,
    group::{GroupSummary, ToolKind, tool_kind},
    view::Chip,
  },
  transcript::{Entry, EntryKind},
};

use super::{Timeline, entry};
use crate::{
  theme::{ConversationSpacing, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::pressable,
  },
};

/// A chip's height (`u0aS4V`), and the rows an open group lists before *Show N more*.
const CHIP_HEIGHT: f32 = 26.0;
const FIRST_ROWS: usize = 7;

/// Each of `chips`, with the rows of an open one under it.
pub(super) fn groups(ctx: &mut Ctx, timeline: &Timeline, chips: &[Chip]) -> Vec<Element> {
  let mut out = Vec::new();
  for chip in chips {
    out.push(closed(ctx, timeline, chip));
    if chip.open {
      out.push(rows(ctx, timeline, chip));
    }
  }
  out
}

/// The chip; pressed, it opens or folds its group.
fn closed(ctx: &Ctx, timeline: &Timeline, chip: &Chip) -> Element {
  let summary = &chip.group.summary;
  // Its command still runs only while its agent works; otherwise it has no end.
  let works = chip
    .group
    .entries
    .last()
    .is_some_and(|entry| timeline.works(entry.invocation));
  let glyph = if summary.running.is_some() && works {
    Icon::LoaderCircle
  } else {
    Icon::Wrench
  };
  let failure = summary.failure().map(|failure| {
    Text::new(&message_text(ctx, &failure))
      .id("conversation-group-failure")
      .variant(ExtraTypography::Note.role())
      .color(PaletteColor::Danger)
      .nowrap()
  });
  let chevron = if chip.open { Icon::ChevronUp } else { Icon::ChevronDown };
  let (toggles, key) = (timeline.toggles.clone(), chip.key.clone());
  pressable(move || toggles.flip(&key, false))
    .id(format!("conversation-group-{}", chip.key))
    .describe("open", chip.open.to_string())
    .max_width(Dimension::Pct(100.0))
    .height(Dimension::Px(CHIP_HEIGHT))
    .padding_horizontal(SpacingSize::Sm)
    .spacing(ExtraSpacing::Snug.role())
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&summary_text(ctx, &chip.group, works))
        .id("conversation-group-summary")
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextSecondary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .min_width(Dimension::Px(0.0))
        .flex_shrink(1.0),
    )
    .with_children(failure)
    .child(icon(chevron, IconSize::Sm, PaletteColor::TextMuted))
    .into()
}

/// The open group's rows: the first seven, or all once *Show N more* was pressed.
fn rows(ctx: &mut Ctx, timeline: &Timeline, chip: &Chip) -> Element {
  // A turn's usage is its step divider's, not a row of its own.
  let entries: Vec<Entry> = chip
    .shown
    .iter()
    .filter(|entry| !matches!(entry.kind, EntryKind::Usage(_)))
    .cloned()
    .collect();
  let all_key: Arc<str> = format!("{}-all", chip.key).into();
  let everything = timeline.toggles.is_open(&all_key, false);
  let (listed, rest) = if everything || entries.len() <= FIRST_ROWS {
    (&entries[..], &[][..])
  } else {
    entries.split_at(FIRST_ROWS)
  };
  let mut rows: Vec<Element> = listed
    .iter()
    .map(|entry| {
      let key: Arc<str> = format!("entry-{}-{}", entry.sequence, entry.part).into();
      entry::tool_row(ctx, timeline, &key, entry)
    })
    .collect();
  if !rest.is_empty() {
    let label = ctx.t_args(
      "conversation.group.more",
      [("count", rest.len().to_string().as_str()), ("kinds", &kinds(ctx, rest))],
    );
    let toggles = timeline.toggles.clone();
    rows.push(
      pressable(move || toggles.flip(&all_key, false))
        .id(format!("conversation-group-more-{}", chip.key))
        .height(Dimension::Px(CHIP_HEIGHT))
        .padding_horizontal(SpacingSize::Sm)
        .child(
          Text::new(&label)
            .variant(TypographyStyle::FieldLabel)
            .color(ExtraColor::AccentText.role())
            .nowrap(),
        )
        .into(),
    );
  }
  Column::new()
    .id(format!("conversation-group-rows-{}", chip.key))
    .width(Dimension::Pct(100.0))
    .padding_left(ConversationSpacing::ToolRows.role())
    .with_children(rows)
    .into()
}

/// *Read 3 files · searched 2 times · ran pnpm test*, in the fixed order of its kinds.
fn summary_text(ctx: &Ctx, group: &ToolGroup, works: bool) -> String {
  let summary = &group.summary;
  let count = |(one, other): (&str, &str), count: usize| {
    if count == 1 {
      ctx.t(one)
    } else {
      ctx.t_args(other, [("count", count.to_string().as_str())])
    }
  };
  let mut parts: Vec<Arc<str>> = Vec::new();
  if summary.reads > 0 {
    parts.push(count(
      ("conversation.group.read.one", "conversation.group.read.other"),
      summary.reads,
    ));
  }
  if summary.searches > 0 {
    parts.push(count(
      ("conversation.group.searched.one", "conversation.group.searched.other"),
      summary.searches,
    ));
  }
  if summary.memory {
    parts.push(ctx.t("conversation.group.memory"));
  }
  parts.extend(commands(ctx, summary));
  if summary.edits > 0 {
    parts.push(count(
      ("conversation.group.edited.one", "conversation.group.edited.other"),
      summary.edits,
    ));
  }
  if summary.mcp > 0 {
    parts.push(count(
      ("conversation.group.mcp.one", "conversation.group.mcp.other"),
      summary.mcp,
    ));
  }
  if let Some(running) = &summary.running {
    let key = if works {
      "conversation.group.running"
    } else {
      "conversation.group.unfinished"
    };
    parts.push(ctx.t_args(key, [("command", &**running)]));
  }
  if parts.is_empty() {
    parts.push(notes(ctx, &group.entries));
  }
  let joined = parts
    .iter()
    .map(|part| &**part)
    .collect::<Vec<_>>()
    .join(&ctx.t("conversation.separator"));
  capitalized(&joined)
}

/// *ran pnpm test*, *ran cargo fmt and cargo test*, or *ran 5 commands*.
fn commands(ctx: &Ctx, summary: &GroupSummary) -> Option<Arc<str>> {
  Some(match summary.named.as_slice() {
    _ if summary.commands == 0 => return None,
    [one] if summary.commands == 1 => ctx.t_args("conversation.group.ranOne", [("command", &**one)]),
    [first, second] if summary.commands == 2 => ctx.t_args(
      "conversation.group.ranTwo",
      [("first", &**first), ("second", &**second)],
    ),
    _ => ctx.t_args(
      "conversation.group.ranMany",
      [("count", summary.commands.to_string().as_str())],
    ),
  })
}

/// What a group of reasoning, usage and unparsed lines alone says.
fn notes(ctx: &Ctx, entries: &[Entry]) -> Arc<str> {
  match entries.first().map(|entry| &entry.kind) {
    Some(EntryKind::Reasoning(_)) => ctx.t("conversation.entry.reasoning"),
    Some(EntryKind::Raw(lines)) if lines.len() == 1 => ctx.t("conversation.entry.unparsed.one"),
    Some(EntryKind::Raw(lines)) => ctx.t_args(
      "conversation.entry.unparsed.other",
      [("count", lines.len().to_string().as_str())],
    ),
    _ => ctx.t("conversation.entry.usage"),
  }
}

/// *4 reads, 1 search*: what `entries` are, by kind.
fn kinds(ctx: &Ctx, entries: &[Entry]) -> String {
  const KEYS: [(&str, &str); 6] = [
    (
      "conversation.group.kinds.reads.one",
      "conversation.group.kinds.reads.other",
    ),
    (
      "conversation.group.kinds.searches.one",
      "conversation.group.kinds.searches.other",
    ),
    (
      "conversation.group.kinds.commands.one",
      "conversation.group.kinds.commands.other",
    ),
    (
      "conversation.group.kinds.edits.one",
      "conversation.group.kinds.edits.other",
    ),
    ("conversation.group.kinds.mcp.one", "conversation.group.kinds.mcp.other"),
    (
      "conversation.group.kinds.notes.one",
      "conversation.group.kinds.notes.other",
    ),
  ];
  let counts = GroupSummary::kinds(entries);
  let notes = entries
    .iter()
    .filter(|entry| matches!(tool_kind(entry), Some(ToolKind::Note) | None))
    .count();
  KEYS
    .iter()
    .zip(counts.into_iter().chain([notes]))
    .filter(|(_, count)| *count > 0)
    .map(|((one, other), count)| {
      if count == 1 {
        ctx.t(one).to_string()
      } else {
        ctx.t_args(other, [("count", count.to_string().as_str())]).to_string()
      }
    })
    .collect::<Vec<_>>()
    .join(&ctx.t("conversation.list.separator"))
}

/// `text` with its first letter in upper case: the summary starts a sentence.
fn capitalized(text: &str) -> String {
  let mut chars = text.chars();
  chars
    .next()
    .map(|first| first.to_uppercase().chain(chars).collect())
    .unwrap_or_default()
}
