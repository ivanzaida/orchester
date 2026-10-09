//! A transcript entry's row. In an open tool group (`Tool row` `uFWI7`): 26 high, 8 at its sides, a 13 glyph
//! in `text_muted`, what it is in mono 12 `text_secondary` (*cargo test -p webhooks*), how it ended in 11
//! (*exit 1 · 9 s*, in `danger` when it failed) and its time in mono 11, 8 apart; how its request was answered
//! on a line under it, 29 in (`Decision mark` `CCpkr`: an 11 shield and 11 `text_muted`). Pressed, a command
//! opens to its output and a file change opens the file in the Changes tab. A problem, never folded into a
//! group (`Problem row` `IxIMv`): on `surface_raised` in a `border` stroke, radius 8, 9 by 12 inside, a 14
//! glyph, what happened in 12/500 and what it means in 11.5/1.45, its time at the end; what it printed under
//! it. What an entry says is [`shown`]'s.

mod change;
mod mcp;
mod native;
mod problem;
mod shown;

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  display,
  tasks::transcript::{Entry, EntryKind},
};

use self::shown::Shown;
use super::Timeline;
use crate::{
  routes::{ChangeSource, ChangedFile, TaskView},
  theme::{
    ConversationRadius, ConversationSpacing, ConversationText, ExtraColor, ExtraSpacing, ExtraTypography, IconSize,
  },
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
  },
};

/// A tool row's height (`uFWI7`), and the most output lines a row shows: its last ones.
const ROW_HEIGHT: f32 = 26.0;
const OUTPUT_LINES: usize = 200;

/// `entry`'s row in an open tool group; `key` keeps whether a person opened it.
pub(super) fn tool_row(ctx: &mut Ctx, timeline: &Timeline, key: &Arc<str>, entry: &Entry) -> Element {
  let shown = shown::shown(ctx, entry, timeline.works(entry.invocation));
  let id = format!("conversation-{key}");
  let open = shown.collapsible && shown.has_details() && timeline.toggles.is_open(key, false);
  let line = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding_horizontal(SpacingSize::Sm)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(shown.glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&shown.title)
        .id(format!("{id}-title"))
        .variant(if shown.code {
          TypographyStyle::Mono
        } else {
          TypographyStyle::Description
        })
        .color(PaletteColor::TextSecondary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .min_width(Dimension::Px(0.0))
        .flex(1.0),
    )
    .with_children((!shown.meta.is_empty()).then(|| {
      Text::new(&shown.meta)
        .id(format!("{id}-meta"))
        .variant(TypographyStyle::Caption)
        .color(if shown.failed {
          PaletteColor::Danger
        } else {
          PaletteColor::TextMuted
        })
        .nowrap()
    }))
    .child(time(entry));
  let line = pressed(ctx, timeline, (key, &shown, open), line);
  let mark = shown.mark.as_ref().map(|words| {
    let glyph = if entry.is_failure() {
      Icon::ShieldAlert
    } else {
      Icon::ShieldCheck
    };
    Row::new()
      .id(format!("{id}-mark"))
      .padding(
        Padding::new()
          .left(ConversationSpacing::Mark.role())
          .bottom(ExtraSpacing::Fine.role()),
      )
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Center)
      .child(icon(glyph, IconSize::Xs, PaletteColor::TextMuted))
      .child(Text::new(words).variant(TypographyStyle::Caption).nowrap())
  });
  let details = open.then(|| {
    Column::new()
      .padding_left(ConversationSpacing::Mark.role())
      .padding_bottom(SpacingSize::Xs)
      .child(details(ctx, &id, &shown))
  });
  Column::new()
    .id(id)
    .describe("open", open.to_string())
    .width(Dimension::Pct(100.0))
    .child(line)
    .with_children(mark)
    .with_children(details)
    .into()
}

/// `line`, pressed: a file change opens its file in the Changes tab, a row with more to show opens or folds.
fn pressed(ctx: &mut Ctx, timeline: &Timeline, (key, shown, open): (&Arc<str>, &Shown, bool), line: Row) -> Element {
  if let Some(path) = &shown.opens
    && let Ok(file) = ChangedFile::new(path.to_string(), ChangeSource::Checkpoint)
  {
    let go = timeline.open(ctx, TaskView::Changes(Some(file)));
    return pressable(go)
      .id(format!("conversation-{key}-open"))
      .width(Dimension::Pct(100.0))
      .child(line)
      .into();
  }
  if !shown.collapsible || !shown.has_details() {
    return line.into();
  }
  let id = format!("conversation-{key}-toggle");
  let (toggles, key) = (timeline.toggles.clone(), key.clone());
  pressable(move || toggles.flip(&key, open))
    .id(id)
    .width(Dimension::Pct(100.0))
    .child(line)
    .into()
}

/// The entry's time, in mono 11 `text_muted`.
fn time(entry: &Entry) -> Text {
  Text::new(&display::clock(entry.at))
    .variant(ExtraTypography::MonoNote.role())
    .nowrap()
}

/// A problem row: never folded, what it printed shown.
pub(super) fn problem_row(ctx: &mut Ctx, timeline: &Timeline, key: &Arc<str>, entry: &Entry) -> Element {
  let agent = timeline
    .detail
    .run
    .as_ref()
    .and_then(|run| run.step_of(entry.execution))
    .and_then(|step| step.agent.clone())
    .unwrap_or_else(|| "".into());
  let shown =
    problem::titled(ctx, entry, &agent).unwrap_or_else(|| shown::shown(ctx, entry, timeline.works(entry.invocation)));
  let id = format!("conversation-problem-{key}");
  let explained = shown
    .body
    .clone()
    .or_else(|| (!shown.meta.is_empty()).then(|| shown.meta.clone()));
  let meta = shown
    .body
    .as_ref()
    .and((!shown.meta.is_empty()).then(|| shown.meta.clone()));
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(&shown.title)
        .id(format!("{id}-title"))
        .variant(TypographyStyle::Label)
        .selectable(true),
    )
    .with_children(explained.map(|words| {
      Text::new(&words)
        .id(format!("{id}-detail"))
        .variant(if shown.mono_body {
          TypographyStyle::Mono
        } else {
          ConversationText::ProblemDetail.role()
        })
        .selectable(true)
    }))
    .with_children(meta.map(|meta| Text::new(&meta).variant(TypographyStyle::Caption)))
    .with_children(
      shown
        .mark
        .as_ref()
        .map(|mark| Text::new(mark).variant(TypographyStyle::Caption)),
    )
    .with_children(shown.has_details().then(|| {
      let output = Shown { body: None, ..shown };
      details(ctx, &id, &output)
    }));
  Row::new()
    .id(id)
    .describe("problem", "true")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      SpacingSize::Md,
      ConversationSpacing::ProblemRow.role(),
    ))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ConversationRadius::ProblemRow.role())
    .child(icon(shown_glyph(entry), IconSize::Md, PaletteColor::TextSecondary))
    .child(text)
    .child(time(entry))
    .into()
}

/// A problem row's glyph: what was denied shows a ban, anything else the alert.
fn shown_glyph(entry: &Entry) -> Icon {
  match &entry.kind {
    EntryKind::Command(_) => Icon::Ban,
    EntryKind::ToolCommand(_) => Icon::Terminal,
    EntryKind::McpServer(..) => Icon::Unplug,
    _ => Icon::CircleAlert,
  }
}

/// What an open row shows under its line: the command whole, the body, the change and the output.
fn details(ctx: &Ctx, id: &str, shown: &Shown) -> Element {
  Column::new()
    .id(format!("{id}-details"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .with_children(
      shown
        .command
        .as_ref()
        .map(|command| code(&format!("{id}-command"), std::slice::from_ref(command), None)),
    )
    .with_children(shown.body.as_ref().map(|body| {
      Text::new(body)
        .variant(if shown.mono_body {
          TypographyStyle::Mono
        } else {
          ExtraTypography::Prose.role()
        })
        .selectable(true)
    }))
    .with_children(shown.change.as_ref().map(|found| change::block(ctx, found, id)))
    .with_children((!shown.output.is_empty()).then(|| output(ctx, id, shown)))
    .into()
}

/// The output block: its last lines when there are many, with how many are left out, and what was not kept
/// of it at all.
fn output(ctx: &Ctx, id: &str, shown: &Shown) -> Element {
  let hidden = shown.output.len().saturating_sub(OUTPUT_LINES);
  let trimmed = (hidden > 0).then(|| {
    ctx.t_args(
      "conversation.entry.outputTrimmed",
      [("count", hidden.to_string().as_str())],
    )
  });
  let notes: Vec<Arc<str>> = shown.omitted.iter().cloned().chain(trimmed).collect();
  code(&format!("{id}-output"), &shown.output[hidden..], Some(notes))
}

/// `lines` in mono on `code_bg`, with `notes` above them.
fn code(id: &str, lines: &[Arc<str>], notes: Option<Vec<Arc<str>>>) -> Element {
  let text = lines.iter().map(|line| &**line).collect::<Vec<_>>().join("\n");
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(ExtraSpacing::Tight.role())
    .background(ExtraColor::CodeBackground.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .with_children(
      notes
        .into_iter()
        .flatten()
        .map(|note| Text::new(&note).variant(TypographyStyle::Caption)),
    )
    .child(
      Text::new(&text)
        .variant(ExtraTypography::MonoNote.role())
        .color(PaletteColor::TextSecondary)
        .selectable(true),
    )
    .into()
}
