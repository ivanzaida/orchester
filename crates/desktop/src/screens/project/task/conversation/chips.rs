//! Tool activity as chips in an agent's message (`kpnmP`): one chip per tool group, in a hairline
//! stroke with radius 6 — a glyph for what it did first (book-open for reads and searches, terminal
//! for commands, file-pen for edits), what the group did (*Read 4 files · searched 2 times*), *N calls*
//! and chevron-right. A failed command turns the glyph into circle-alert in `$text`; a group whose
//! command still runs is on `$raised` with `$border`, in `$text`, and ends with a loader. A chip opens
//! its group in place (`bdCsu`): a bordered list under its header (wrench, the summary in `$text`,
//! chevron-down) of the group's entries (`N4SlQq`, with their time), at most seven before *Show N
//! more · 4 reads, 1 search*; with a search, the matching ones, how many match, and *Show N without
//! a match* (`Hn1xv`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use super::{
  super::activity::{EntryProps, EntryView},
  Timeline,
};
use crate::{
  tasks::{
    timeline::{
      ToolGroup,
      group::{GroupSummary, ToolKind, tool_kind},
      view::Chip,
    },
    transcript::{Entry, EntryKind},
  },
  theme::{ExtraColor, ExtraSpacing},
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{pressable, surface},
  },
};

/// An open group's header and *Show N more* (`xy1RN`, `h3WMJ`), and the entries it shows before it.
const HEADER_HEIGHT: f32 = 32.0;
const MORE_HEIGHT: f32 = 30.0;
const FIRST_ENTRIES: usize = 7;

/// `chips` under an agent's message: the closed ones side by side, wrapping; an open one across the
/// row, its entries in place.
pub(super) fn chips(ctx: &mut Ctx, timeline: &Timeline, chips: &[Chip], raw: bool) -> Vec<Element> {
  let mut out = Vec::new();
  let mut closed: Vec<Element> = Vec::new();
  let line = |closed: &mut Vec<Element>| {
    Row::new()
      .width(Dimension::Pct(100.0))
      .wrap()
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .with_children(std::mem::take(closed))
      .into()
  };
  for chip in chips {
    if chip.open {
      if !closed.is_empty() {
        out.push(line(&mut closed));
      }
      out.push(open_group(ctx, timeline, chip, raw));
    } else {
      closed.push(closed_chip(ctx, timeline, chip));
    }
  }
  if !closed.is_empty() {
    out.push(line(&mut closed));
  }
  out
}

/// A closed chip; pressed, it opens its group.
fn closed_chip(ctx: &Ctx, timeline: &Timeline, chip: &Chip) -> Element {
  let summary = &chip.group.summary;
  let running = summary.running.is_some();
  let tone = if running {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  let glyph = match &summary.failed {
    Some(_) => icon(Icon::AlertCircle, IconSize::Small, PaletteColor::TextPrimary),
    None => icon(glyph_of(&chip.group), IconSize::Small, tone.clone()),
  };
  let text = Text::new(&summary_text(ctx, &chip.group))
    .id("conversation-group-summary")
    .variant(TypographyStyle::Caption)
    .color(tone.clone())
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .min_width(Dimension::Px(0.0))
    .flex_shrink(1.0);
  let end: Element = if running {
    icon(Icon::Loader, IconSize::Small, tone).into()
  } else {
    Row::new()
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      // Reasoning, usage and unparsed lines alone are no calls: their summary counts them.
      .with_children((summary.calls > 0).then(|| {
        Text::new(&calls(ctx, summary.calls))
          .variant(TypographyStyle::Mono)
          .color(PaletteColor::TextMuted)
          .nowrap()
      }))
      .child(icon(Icon::ChevronRight, IconSize::Small, PaletteColor::TextMuted))
      .into()
  };
  let (toggles, key) = (timeline.toggles.clone(), chip.key.clone());
  let chip = pressable(surface().id("conversation-group-toggle"), move || {
    toggles.flip(&key, false)
  })
  .describe("open", "false")
  .max_width(Dimension::Pct(100.0))
  .padding_vertical(ExtraSpacing::Chip.role())
  .padding_horizontal(ExtraSpacing::Inline.role())
  .spacing(ExtraSpacing::Tight.role())
  .align_items(Alignment::Center)
  .rounded(RadiusSize::Sm)
  .child(glyph)
  .child(text)
  .child(end);
  if running {
    chip
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .into()
  } else {
    chip
      .border(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .into()
  }
}

/// What a chip's glyph shows: the kind its summary names first.
fn glyph_of(group: &ToolGroup) -> Icon {
  let summary = &group.summary;
  if summary.running.is_some() || (summary.reads + summary.searches == 0 && !summary.memory && summary.commands > 0) {
    return Icon::Terminal;
  }
  if summary.reads + summary.searches > 0 || summary.memory {
    return Icon::BookOpen;
  }
  if summary.edits > 0 {
    return Icon::FilePen;
  }
  if summary.mcp > 0 {
    return Icon::Plug;
  }
  Icon::Wrench
}

/// An open group in place: its header, which closes it, then its entries.
fn open_group(ctx: &mut Ctx, timeline: &Timeline, chip: &Chip, raw: bool) -> Element {
  let header = header(ctx, timeline, chip);
  let entries = entries(ctx, timeline, chip, raw);
  Column::new()
    .id(format!("conversation-group-{}", chip.key))
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .describe("open", "true")
    .child(header)
    .with_children(entries)
    .into()
}

fn header(ctx: &Ctx, timeline: &Timeline, chip: &Chip) -> Element {
  let group = &chip.group;
  let glyph = if group.summary.failed.is_some() {
    Icon::AlertCircle
  } else {
    Icon::Wrench
  };
  let matching = (!chip.left_out.is_empty()).then(|| {
    let total = chip.shown.len() + chip.left_out.len();
    let key = if total == 1 {
      "runs.conversation.chip.matching.one"
    } else {
      "runs.conversation.chip.matching.other"
    };
    Text::new(&ctx.t_args(
      key,
      [
        ("matching", chip.shown.len().to_string().as_str()),
        ("count", total.to_string().as_str()),
      ],
    ))
    .id("conversation-group-matching")
    .variant(TypographyStyle::Mono)
    .color(PaletteColor::TextMuted)
    .nowrap()
  });
  let (toggles, key) = (timeline.toggles.clone(), chip.key.clone());
  pressable(surface().id("conversation-group-toggle"), move || {
    toggles.flip(&key, true)
  })
  .width(Dimension::Pct(100.0))
  .height(Dimension::Px(HEADER_HEIGHT))
  .padding_horizontal(ExtraSpacing::Inline.role())
  .spacing(SpacingSize::Sm)
  .align_items(Alignment::Center)
  .child(icon(glyph, IconSize::Small, PaletteColor::TextPrimary))
  .child(
    Text::new(&summary_text(ctx, group))
      .id("conversation-group-summary")
      .variant(TypographyStyle::Caption)
      .color(PaletteColor::TextPrimary)
      .nowrap()
      .text_overflow(TextOverflow::Elipsis)
      .min_width(Dimension::Px(0.0))
      .flex_shrink(1.0),
  )
  .with_children(matching)
  .child(Spacer::new().flex(1.0))
  .child(icon(Icon::ChevronDown, IconSize::Small, PaletteColor::TextPrimary))
  .into()
}

/// The open group's entries: the first seven, or all once *Show N more* was pressed; with a search
/// the matching ones, or all once *Show N without a match* was.
fn entries(ctx: &mut Ctx, timeline: &Timeline, chip: &Chip, raw: bool) -> Vec<Element> {
  let (shown, left_out) = (&chip.shown, &chip.left_out);
  let all_key: Arc<str> = format!("{}-all", chip.key).into();
  let everything = timeline.toggles.is_open(&all_key, false);
  let (listed, rest): (Vec<Entry>, Vec<Entry>) = if !left_out.is_empty() {
    if everything {
      let mut all: Vec<Entry> = shown.iter().chain(left_out).cloned().collect();
      all.sort_by_key(|entry| (entry.sequence, entry.part));
      (all, Vec::new())
    } else {
      (shown.clone(), left_out.clone())
    }
  } else if everything || shown.len() <= FIRST_ENTRIES {
    (shown.clone(), Vec::new())
  } else {
    (shown[..FIRST_ENTRIES].to_vec(), shown[FIRST_ENTRIES..].to_vec())
  };
  let mut rows: Vec<Element> = listed
    .into_iter()
    .map(|entry| {
      let entry_key: Arc<str> = format!("entry-{}-{}", entry.sequence, entry.part).into();
      let view = ctx.mount_keyed::<EntryView>(
        &entry_key,
        EntryProps {
          key: entry_key.clone(),
          task: timeline.task,
          entry,
          agent: "".into(),
          raw,
          time_column: true,
          in_group: true,
          toggles: timeline.toggles.clone(),
          request: None,
        },
      );
      ruled(view)
    })
    .collect();
  if !rest.is_empty() {
    let label_key = if left_out.is_empty() {
      "runs.conversation.group.more"
    } else {
      "runs.conversation.group.withoutMatch"
    };
    let label = ctx.t_args(
      label_key,
      [
        ("count", rest.len().to_string().as_str()),
        ("kinds", &kinds(ctx, &rest)),
      ],
    );
    let toggles = timeline.toggles.clone();
    rows.push(ruled(
      pressable(surface().id("conversation-group-more"), move || {
        toggles.flip(&all_key, false)
      })
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(MORE_HEIGHT))
      .align_items(Alignment::Center)
      .child(
        Text::new(&label)
          .variant(TypographyStyle::Link)
          .color(PaletteColor::TextPrimary)
          .nowrap(),
      )
      .into(),
    ));
  }
  rows
}

/// A line of the open group, under a hairline, with the group's side padding (`H1sjek`).
fn ruled(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(content)
    .into()
}

/// *2 calls*.
fn calls(ctx: &Ctx, calls: usize) -> Arc<str> {
  let key = if calls == 1 {
    "runs.conversation.chip.calls.one"
  } else {
    "runs.conversation.chip.calls.other"
  };
  ctx.t_args(key, [("count", calls.to_string().as_str())])
}

/// *Read 3 files · searched 2 times · ran pnpm test · exit 1*, in the fixed order of its kinds.
fn summary_text(ctx: &Ctx, group: &ToolGroup) -> String {
  let summary = &group.summary;
  let count = |key: &str, count: usize| {
    let key = format!("{key}.{}", if count == 1 { "one" } else { "other" });
    ctx.t_args(&key, [("count", count.to_string().as_str())])
  };
  let mut parts: Vec<Arc<str>> = Vec::new();
  if summary.reads > 0 {
    parts.push(count("runs.conversation.group.read", summary.reads));
  }
  if summary.searches > 0 {
    parts.push(count("runs.conversation.group.searched", summary.searches));
  }
  if summary.memory {
    parts.push(ctx.t("runs.conversation.group.memory"));
  }
  parts.extend(commands(ctx, summary));
  if summary.edits > 0 {
    parts.push(count("runs.conversation.group.edited", summary.edits));
  }
  if summary.mcp > 0 {
    parts.push(count("runs.conversation.group.mcp", summary.mcp));
  }
  if let Some(running) = &summary.running {
    parts.push(ctx.t_args("runs.conversation.group.running", [("command", &**running)]));
  }
  if parts.is_empty() {
    parts.push(notes(ctx, &group.entries));
  }
  let joined = parts
    .iter()
    .map(|part| &**part)
    .collect::<Vec<_>>()
    .join(&ctx.t("runs.conversation.group.separator"));
  capitalized(&joined)
}

/// *ran pnpm test*, *ran cargo fmt and cargo test*, or *ran 5 commands*; a failed one with its exit code.
fn commands(ctx: &Ctx, summary: &GroupSummary) -> Vec<Arc<str>> {
  if summary.commands == 0 {
    return Vec::new();
  }
  let ran = match summary.named.as_slice() {
    [one] if summary.commands == 1 => ctx.t_args("runs.conversation.group.ranOne", [("command", &**one)]),
    [first, second] if summary.commands == 2 => ctx.t_args(
      "runs.conversation.group.ranTwo",
      [("first", &**first), ("second", &**second)],
    ),
    _ => ctx.t_args(
      "runs.conversation.group.ranMany",
      [("count", summary.commands.to_string().as_str())],
    ),
  };
  let mut parts = vec![ran];
  parts.extend(summary.failure().map(|failure| message_text(ctx, &failure)));
  parts
}

/// What a group of reasoning, usage and unparsed lines alone says.
fn notes(ctx: &Ctx, entries: &[Entry]) -> Arc<str> {
  match entries.first().map(|entry| &entry.kind) {
    Some(EntryKind::Reasoning(_)) => ctx.t("runs.activity.reasoning"),
    Some(EntryKind::Raw(lines)) => {
      let key = if lines.len() == 1 {
        "runs.activity.unparsed.one"
      } else {
        "runs.activity.unparsed.other"
      };
      ctx.t_args(key, [("count", lines.len().to_string().as_str())])
    }
    _ => ctx.t("runs.activity.usage"),
  }
}

/// *4 reads, 1 search*: what `entries` are, by kind.
fn kinds(ctx: &Ctx, entries: &[Entry]) -> String {
  const KEYS: [&str; 5] = [
    "runs.conversation.group.kinds.reads",
    "runs.conversation.group.kinds.searches",
    "runs.conversation.group.kinds.commands",
    "runs.conversation.group.kinds.edits",
    "runs.conversation.group.kinds.mcp",
  ];
  let counts = GroupSummary::kinds(entries);
  let others = entries
    .iter()
    .filter(|entry| matches!(tool_kind(entry), Some(ToolKind::Note) | None))
    .count();
  KEYS
    .iter()
    .zip(counts)
    .chain(std::iter::once((&"runs.conversation.group.kinds.other", others)))
    .filter(|(_, count)| *count > 0)
    .map(|(key, count)| {
      let key = format!("{key}.{}", if count == 1 { "one" } else { "other" });
      ctx.t_args(&key, [("count", count.to_string().as_str())]).to_string()
    })
    .collect::<Vec<_>>()
    .join(&ctx.t("common.listSeparator"))
}

/// `text` with its first letter in upper case: the summary starts a sentence.
fn capitalized(text: &str) -> String {
  let mut chars = text.chars();
  chars
    .next()
    .map(|first| first.to_uppercase().chain(chars).collect())
    .unwrap_or_default()
}
