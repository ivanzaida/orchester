//! A tool group (*Conversation / Tool group* `uoK7y`): a 32-high row indented to the message text,
//! in a hairline stroke with radius 7 — wrench, what the group did (*Read 3 files · searched 2 times
//! · ran pnpm test*), *N tool calls · 12:01–12:04* and the disclosure. A failed command turns the
//! glyph into circle-alert in `$text`; a group whose command still runs ends with it and a loader.
//! Open, the header turns `$text` and the entries follow (`N4SlQq`, with their time) inside the same
//! stroke, at most seven before *Show N more · 4 reads, 1 search*; with a search, the matching ones
//! and *Show N without a match* (`a5hF4`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};

use super::{
  super::activity::{EntryProps, EntryView},
  Timeline,
};
use crate::{
  display,
  tasks::{
    timeline::{
      ToolGroup,
      group::{GroupSummary, ToolKind, tool_kind},
    },
    transcript::{Entry, EntryKind},
  },
  theme::{ExtraColor, ExtraSpacing},
  ui::{
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
  },
};

/// Header height (`uoK7y`) and the entries an open group shows before *Show N more*.
const HEADER_HEIGHT: f32 = 32.0;
const FIRST_ENTRIES: usize = 7;

pub(super) fn tool_group(
  ctx: &mut Ctx,
  timeline: &Timeline,
  (key, group, open): (&Arc<str>, &ToolGroup, bool),
  (shown, left_out): (&[Entry], &[Entry]),
  raw: bool,
) -> Element {
  let header = header(ctx, timeline, (key, group, open));
  let entries = open.then(|| entries(ctx, timeline, key, (shown, left_out), raw));
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_left(ExtraSpacing::TimelineIndent.role())
    .child(
      Column::new()
        .id(format!("conversation-group-{key}"))
        .width(Dimension::Pct(100.0))
        .border(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
        .rounded(RadiusSize::Md)
        .describe("open", open.to_string())
        .child(header)
        .with_children(entries),
    )
    .into()
}

fn header(ctx: &Ctx, timeline: &Timeline, (key, group, open): (&Arc<str>, &ToolGroup, bool)) -> Element {
  let summary = &group.summary;
  let glyph = if summary.running.is_some() {
    icon(Icon::Loader, IconSize::Small, PaletteColor::TextSecondary)
  } else if summary.failed.is_some() {
    icon(Icon::AlertCircle, IconSize::Small, PaletteColor::TextPrimary)
  } else {
    icon(Icon::Wrench, IconSize::Small, PaletteColor::TextSecondary)
  };
  let text = Text::new(&summary_text(ctx, group))
    .id("conversation-group-summary")
    .variant(TypographyStyle::FieldLabel)
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .min_width(Dimension::Px(0.0))
    .flex(1.0);
  let text = if open {
    text.color(PaletteColor::TextPrimary)
  } else {
    text
  };
  let chevron = if open { Icon::ChevronDown } else { Icon::ChevronRight };
  let (toggles, key) = (timeline.toggles.clone(), key.clone());
  pressable(surface().id("conversation-group-toggle"), move || {
    toggles.flip(&key, open)
  })
  .width(Dimension::Pct(100.0))
  .height(Dimension::Px(HEADER_HEIGHT))
  .padding_horizontal(ExtraSpacing::Inline.role())
  .spacing(SpacingSize::Sm)
  .align_items(Alignment::Center)
  .child(glyph)
  .child(text)
  .child(
    Text::new(&meta(ctx, summary))
      .variant(TypographyStyle::Mono)
      .nowrap()
      .flex_shrink(0.0),
  )
  .child(icon(chevron, IconSize::Small, PaletteColor::TextMuted))
  .into()
}

/// The open group's entries: the first seven, or all once *Show N more* was pressed; with a search
/// the matching ones, or all once *Show N without a match* was.
fn entries(
  ctx: &mut Ctx,
  timeline: &Timeline,
  key: &Arc<str>,
  (shown, left_out): (&[Entry], &[Entry]),
  raw: bool,
) -> Element {
  let all_key: Arc<str> = format!("{key}-all").into();
  let everything = timeline.toggles.is_open(&all_key, false);
  let (listed, rest): (Vec<Entry>, Vec<Entry>) = if !left_out.is_empty() {
    if everything {
      let mut all: Vec<Entry> = shown.iter().chain(left_out).cloned().collect();
      all.sort_by_key(|entry| (entry.sequence, entry.part));
      (all, Vec::new())
    } else {
      (shown.to_vec(), left_out.to_vec())
    }
  } else if everything || shown.len() <= FIRST_ENTRIES {
    (shown.to_vec(), Vec::new())
  } else {
    (shown[..FIRST_ENTRIES].to_vec(), shown[FIRST_ENTRIES..].to_vec())
  };
  let rows: Vec<Element> = listed
    .into_iter()
    .map(|entry| {
      let entry_key: Arc<str> = format!("entry-{}-{}", entry.sequence, entry.part).into();
      ctx.mount_keyed::<EntryView>(
        &entry_key,
        EntryProps {
          key: entry_key.clone(),
          task: timeline.task,
          entry,
          agent: "".into(),
          raw,
          time_column: true,
          toggles: timeline.toggles.clone(),
        },
      )
    })
    .collect();
  let more = (!rest.is_empty()).then(|| {
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
    Element::from(
      pressable(surface().id("conversation-group-more"), move || {
        toggles.flip(&all_key, false)
      })
      .padding_vertical(SpacingSize::Sm)
      .child(Text::new(&label).variant(TypographyStyle::Link).nowrap()),
    )
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_horizontal(ExtraSpacing::Inline.role())
    .padding_bottom(SpacingSize::Sm)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .with_children(rows)
    .with_children(more)
    .into()
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
  if let Some((command, code)) = &summary.failed {
    let code = code.map(|code| code.to_string()).unwrap_or_default();
    parts.push(if summary.named.contains(command) && summary.commands <= 2 {
      ctx.t_args("runs.conversation.group.exit", [("code", code.as_str())])
    } else {
      ctx.t_args(
        "runs.conversation.group.failedCommand",
        [("command", &**command), ("code", code.as_str())],
      )
    });
  }
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

/// *12 tool calls · 12:01–12:04*.
fn meta(ctx: &Ctx, summary: &GroupSummary) -> Arc<str> {
  let first = summary.first.map(display::clock).unwrap_or_default();
  let last = summary.last.map(display::clock).unwrap_or_default();
  let times = if first == last {
    first
  } else {
    ctx
      .t_args(
        "runs.conversation.group.span",
        [("first", first.as_str()), ("last", last.as_str())],
      )
      .to_string()
  };
  match summary.calls {
    0 => times.into(),
    1 => ctx.t_args("runs.conversation.group.calls.one", [("times", times.as_str())]),
    calls => ctx.t_args(
      "runs.conversation.group.calls.other",
      [("count", calls.to_string().as_str()), ("times", times.as_str())],
    ),
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
