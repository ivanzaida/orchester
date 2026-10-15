//! A reviewed diff, one card per file (`src/webhooks/sender.rs diff` `QSaSz`): on `code_bg` in a radius 6
//! `border` stroke, the path in Geist Mono 12, 10 inside; each hunk's range on `hover`, 26 high, 12 from the side,
//! wrapping where it is longer than the card (`Diff hunk` `sphVU`); each line in Geist Mono 12, 2 over and under and 12
//! at its sides, a context line in `text_secondary`, a removed one in `danger` on `diff_remove` and an added one in
//! `success` on `diff_add`. Long lines wrap. A card with more rows than fit a page shows them in a scrolling list of
//! its own.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, VirtualizedList},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::diff::{Diff, FileDiff, LineKind};
use orchester_workspace::tasks::local_integration::Preview;

use crate::theme::{ExtraColor, ExtraTypography, LocalText};

/// A hunk's range row, at least (`sphVU`).
const HUNK_HEIGHT: f32 = 26.0;
/// Rows a card lays out in full; past them its rows scroll in a list `LIST_HEIGHT` high.
const SHOWN_ROWS: usize = 160;
const LIST_HEIGHT: f32 = 480.0;

/// What the reviewed text is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Source<'a> {
  /// A unified diff, of any number of files.
  Diff,
  /// The file at this path of a unified diff of several; all of them if none is at it.
  File(&'a str),
  /// The whole content of the new file at this path.
  NewFile(&'a str),
}

/// The cards of `shown`, ids starting with `id`; `path` names the card of content that has no text to show.
pub(super) fn cards(ctx: &mut Ctx, id: &str, shown: &Preview, source: Source<'_>, path: Option<&str>) -> Vec<Element> {
  let text = match shown {
    Preview::Text(text) => text,
    other => {
      let key = match other {
        Preview::Binary => "local_integration.binary",
        Preview::Sensitive => "local_integration.sensitive",
        _ => "local_integration.absent",
      };
      let rows = vec![DiffRow::Note(ctx.t(key))];
      return vec![card(ctx, &format!("{id}-file-0"), path.unwrap_or_default(), rows)];
    }
  };
  let files = match source {
    Source::NewFile(path) => vec![(path.to_owned(), new_file(ctx, text))],
    Source::Diff | Source::File(_) => match Diff::parse(text) {
      Ok(diff) => {
        let only = match source {
          Source::File(only) if diff.files().iter().any(|file| file.path() == only) => Some(only),
          _ => None,
        };
        diff
          .files()
          .iter()
          .filter(|file| only.is_none_or(|only| file.path() == only))
          .map(|file| (file.path().to_owned(), file_rows(ctx, file)))
          .collect()
      }
      Err(_) => vec![(
        path.unwrap_or_default().to_owned(),
        vec![DiffRow::Note(ctx.t("local_integration.unreadableDiff"))],
      )],
    },
  };
  files
    .into_iter()
    .enumerate()
    .map(|(index, (path, rows))| card(ctx, &format!("{id}-file-{index}"), &path, rows))
    .collect()
}

fn file_rows(ctx: &Ctx, file: &FileDiff) -> Vec<DiffRow> {
  let mut rows = Vec::new();
  if file.is_binary() {
    rows.push(DiffRow::Note(ctx.t("local_integration.binary")));
  }
  for hunk in file.hunks() {
    let old = format!("{},{}", hunk.old_start(), hunk.old_count());
    let new = format!("{},{}", hunk.new_start(), hunk.new_count());
    rows.push(DiffRow::Hunk(ctx.t_args(
      "local_integration.hunk",
      [
        ("old", old.as_str()),
        ("new", new.as_str()),
        ("section", hunk.section()),
      ],
    )));
    for line in hunk.lines() {
      rows.push(line_row(ctx, line.kind(), line.text()));
      if line.is_cut() {
        rows.push(DiffRow::Note(ctx.t("local_integration.lineCut")));
      }
    }
  }
  if file.omitted_lines() > 0 {
    rows.push(DiffRow::Note(ctx.t_args(
      "local_integration.omitted",
      [("count", file.omitted_lines().to_string().as_str())],
    )));
  }
  rows
}

fn new_file(ctx: &Ctx, text: &str) -> Vec<DiffRow> {
  let count = text.lines().count().to_string();
  std::iter::once(DiffRow::Hunk(
    ctx.t_args("local_integration.newFileHunk", [("count", count.as_str())]),
  ))
  .chain(text.lines().map(|line| line_row(ctx, LineKind::Added, line)))
  .collect()
}

fn line_row(ctx: &Ctx, kind: LineKind, text: &str) -> DiffRow {
  let key = match kind {
    LineKind::Context => "local_integration.lineContext",
    LineKind::Added => "local_integration.lineAdded",
    LineKind::Removed => "local_integration.lineRemoved",
  };
  DiffRow::Line(kind, ctx.t_args(key, [("text", text)]))
}

fn card(ctx: &mut Ctx, id: &str, path: &str, rows: Vec<DiffRow>) -> Element {
  let head = Row::new()
    .id(format!("{id}-path"))
    .width(Dimension::Pct(100.0))
    .padding(crate::theme::ExtraSpacing::Loose.role())
    .child(
      Text::new(path)
        .variant(TypographyStyle::Mono)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    );
  let body: Element = if rows.len() <= SHOWN_ROWS {
    Column::new()
      .width(Dimension::Pct(100.0))
      .with_children(
        rows
          .into_iter()
          .enumerate()
          .map(|(index, row)| row.element(format!("{id}-row-{index}"))),
      )
      .into()
  } else {
    let rows = rows
      .into_iter()
      .enumerate()
      .map(|(index, row)| RowProps {
        id: format!("{id}-row-{index}"),
        row,
      })
      .collect::<Vec<_>>();
    VirtualizedList::new_shared(ctx, Arc::new(rows))
      .list_key(format!("{id}-rows"))
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(LIST_HEIGHT))
      .mount_keyed::<RowView, _, _, _>(|props| props.id.clone(), RowProps::clone)
  };
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .background(ExtraColor::CodeBackground.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .clip()
    .child(head)
    .child(body)
    .into()
}

/// One row of a card.
#[derive(Clone, Debug, PartialEq)]
enum DiffRow {
  Hunk(Arc<str>),
  Line(LineKind, Arc<str>),
  /// What the card cannot show as lines, e.g. binary content.
  Note(Arc<str>),
}

/// A row as a card lays it out, under its element id.
#[derive(Clone, Debug, PartialEq)]
struct RowProps {
  id: String,
  row: DiffRow,
}

impl DevtoolsInspectable for RowProps {}

impl DiffRow {
  fn element(self, id: String) -> Element {
    let row: Element = match self {
      // A range with a long section wraps inside the card, as code lines do (`s4gj9` at 960).
      DiffRow::Hunk(range) => Row::new()
        .width(Dimension::Pct(100.0))
        .min_height(Dimension::Px(HUNK_HEIGHT))
        .padding(Padding::symmetric(
          SpacingSize::Md,
          crate::theme::ExtraSpacing::Tight.role(),
        ))
        .align_items(Alignment::Center)
        .background(ExtraColor::Hover.role())
        .child(
          Text::new(&range)
            .variant(ExtraTypography::MonoKey.role())
            .flex(1.0)
            .min_width(Dimension::Px(0.0)),
        )
        .into(),
      DiffRow::Line(kind, text) => {
        let (background, color, name) = match kind {
          LineKind::Context => (ExtraColor::CodeBackground, PaletteColor::TextSecondary, "context"),
          LineKind::Added => (ExtraColor::DiffAdded, PaletteColor::Success, "added"),
          LineKind::Removed => (ExtraColor::DiffRemoved, PaletteColor::Danger, "removed"),
        };
        Row::new()
          .width(Dimension::Pct(100.0))
          .padding(Padding::symmetric(
            SpacingSize::Md,
            crate::theme::ExtraSpacing::Tight.role(),
          ))
          .background(background.role())
          .describe("kind", name)
          .child(
            Text::new(&text)
              .variant(TypographyStyle::Mono)
              .color(color)
              .selectable(true)
              .flex(1.0)
              .min_width(Dimension::Px(0.0)),
          )
          .into()
      }
      DiffRow::Note(text) => Row::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
        .child(
          Text::new(&text)
            .variant(LocalText::Note.role())
            .flex(1.0)
            .min_width(Dimension::Px(0.0)),
        )
        .into(),
    };
    row.id(id)
  }
}

struct RowView;

impl Component for RowView {
  type Props = RowProps;

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    props.row.element(props.id)
  }
}
