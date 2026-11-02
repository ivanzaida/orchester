//! A call's row where the search found places (13d `yxLHS`, 13e `z2ai2h`): its line with the places in its command or
//! path marked (`Match`, `Match · current` in 13f `aoVlE`), and, open, its output around the places (`Output`
//! `o9RdGz`): on `code_bg` in a `border` stroke, radius 6, 8 over and under and 12 at its sides, 8 between its head and
//! its lines; the head says how many lines the output has and which it shows in 11 `text_muted`, and at its end *Show
//! all output* in 11/500 `text_secondary` opens the whole output as it shows without a search. A line is its number
//! in a 28 column, right-aligned in mono 11 `text_muted`, then 12 apart its text in mono 12 `text_secondary`, the
//! places in it marked.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, text_style::TextAlign},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::tasks::{
  timeline::find::Spot,
  transcript::{Entry, EntryKind},
};

use super::{OUTPUT_LINES, Timeline, shown::Shown};
use crate::{
  screens::task::conversation::search::marks::{Place, marked},
  theme::{ExtraColor, ExtraTypography},
  ui::pressable::pressable,
};

/// The width of a line's number (`ysxg6` in `Output line 94`).
const NUMBER_WIDTH: f32 = 28.0;
/// Lines shown before the first place and after the last (`lines 94–102` around places on 97 and 100).
const BEFORE: usize = 3;
const AFTER: usize = 2;

/// The places found in a call: where each is, its bytes, and whether it is the current one.
pub(super) type Places = [(Spot, Place)];

/// The call's line `title` (and the path it `opens`) with the places in it marked, in `look`'s style and color; `None`
/// when it holds none, or does not show the text they were found in.
pub(super) fn title(
  entry: &Entry,
  (title, opens): (&str, Option<&Arc<str>>),
  places: &Places,
  look: (TypographyStyle, PaletteColor),
) -> Option<Row> {
  let found: Vec<Place> = places
    .iter()
    .filter(|(spot, _)| *spot == Spot::Title)
    .map(|(_, place)| place.clone())
    .collect();
  if found.is_empty() {
    return None;
  }
  // The places count their bytes in the command as one line, or in the path a file change shows.
  let base = match &entry.kind {
    EntryKind::Command(command) if command.native.is_none() => Some(0),
    EntryKind::ToolCommand(_) => Some(0),
    EntryKind::FileChange(_) => opens.and_then(|path| title.find(&**path)),
    _ => None,
  }?;
  Some(
    Row::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .align_items(Alignment::Center)
      .with_children(marked(title, look, (base, &found))),
  )
}

/// The output of the call `key` around the places found in it, while it has any and the person did not open all of
/// it; `id` names the row.
pub(super) fn output(
  ctx: &Ctx,
  timeline: &Timeline,
  (id, key): (&str, &Arc<str>),
  (entry, shown): (&Entry, &Shown),
  places: &Places,
) -> Option<Element> {
  let lines: Vec<(usize, Place)> = places
    .iter()
    .filter_map(|(spot, place)| match spot {
      Spot::Output(line) => Some((*line, place.clone())),
      Spot::Said | Spot::Title | Spot::Detail => None,
    })
    .collect();
  let (first, last) = (
    lines.iter().map(|(line, _)| *line).min()?,
    lines.iter().map(|(line, _)| *line).max()?,
  );
  let all: Arc<str> = format!("{key}-output-all").into();
  if timeline.toggles.is_open(&all, false) || shown.output.is_empty() {
    return None;
  }
  // At most `OUTPUT_LINES` show: when the places spread wider, the window holds the current one, or else the first.
  let anchor = lines
    .iter()
    .find(|(_, (_, current))| *current)
    .map_or(first, |(line, _)| *line);
  let start = first
    .saturating_sub(BEFORE)
    .max((anchor + AFTER + 1).saturating_sub(OUTPUT_LINES));
  let end = (last + AFTER + 1).min(shown.output.len()).min(start + OUTPUT_LINES);
  let head = head(ctx, (id, &all, timeline), (shown.output.len(), start, end));
  let rows = (start..end).map(|at| {
    let found: Vec<Place> = lines
      .iter()
      .filter(|(line, _)| *line == at)
      .map(|(_, place)| place.clone())
      .collect();
    line(entry, at, &shown.output[at], &found)
  });
  Some(
    Column::new()
      .id(format!("{id}-output"))
      .describe("around", "true")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
      .spacing(SpacingSize::Sm)
      .background(ExtraColor::CodeBackground.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .child(head)
      .child(Column::new().width(Dimension::Pct(100.0)).with_children(rows))
      .into(),
  )
}

/// *Output · 214 lines · lines 94–102, around the matches* and *Show all output* (`u5XNKl`).
fn head(
  ctx: &Ctx,
  (id, all, timeline): (&str, &Arc<str>, &Timeline),
  (total, start, end): (usize, usize, usize),
) -> Element {
  let words = ctx.t_args(
    "conversation.search.outputAround",
    [
      ("total", total.to_string().as_str()),
      ("first", (start + 1).to_string().as_str()),
      ("last", end.to_string().as_str()),
    ],
  );
  let (toggles, all) = (timeline.toggles.clone(), all.clone());
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&words).variant(TypographyStyle::Caption).nowrap())
    .child(Spacer::new().flex(1.0))
    .child(
      pressable(move || toggles.flip(&all, false))
        .id(format!("{id}-output-all"))
        .child(
          Text::new(&ctx.t("conversation.search.showAllOutput"))
            .variant(TypographyStyle::Link)
            .color(PaletteColor::TextSecondary)
            .nowrap(),
        ),
    )
    .into()
}

/// The output's line `at` (from 0), `text` as the row shows it, with the places `found` in it.
fn line(entry: &Entry, at: usize, text: &str, found: &[Place]) -> Element {
  // An unparsed line of standard error shows with words around it: its places count in the line as written.
  let base = match &entry.kind {
    EntryKind::Raw(lines) => lines.get(at).and_then(|(_, line)| text.find(&**line)).unwrap_or(0),
    _ => 0,
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(&(at + 1).to_string())
        .variant(ExtraTypography::MonoNote.role())
        .text_align(TextAlign::Right)
        .width(Dimension::Px(NUMBER_WIDTH))
        .flex_shrink(0.0)
        .nowrap(),
    )
    .child(
      Row::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .align_items(Alignment::Center)
        .with_children(marked(
          text,
          (TypographyStyle::Mono, PaletteColor::TextSecondary),
          (base, found),
        )),
    )
    .into()
}
