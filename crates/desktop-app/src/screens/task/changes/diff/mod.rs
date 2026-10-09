//! The selected file's diff (`Diff` `RsPIH` on `code_bg`): its head (`File head` `rIRdw`: the change's
//! letter, the path, the added and removed counts and where the change was read) and its rows in a
//! virtualised list that also scrolls sideways for long lines. The lines show plain at once and
//! highlighted when a background thread finished them; highlighted files are kept per diff while the
//! view is open, so going back to a file does not highlight it again.

mod line;
mod rows;

use std::{collections::HashMap, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text, VirtualizedList},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::diff::{Diff, FileDiff};
use orchester_workspace::{
  background,
  highlight::{FileTokens, highlight},
  messages::Message,
};
use parking_lot::Mutex;

use self::{
  line::{DiffRowView, RowProps, content_width},
  rows::{DiffRow, rows},
};
use super::files::{FileSummary, change_letter};
use crate::theme::{CodeText, ExtraColor, ExtraSpacing, ExtraTypography};

/// Height of the file head (`rIRdw`).
const HEAD_HEIGHT: f32 = 40.0;
/// Width changes below this are layout noise, not a resize.
const WIDTH_EPSILON: f32 = 0.5;

/// The rows of one file of one diff: plain, or highlighted; with room for a horizontal scrollbar under
/// them, or not.
type RowsKey = (Arc<str>, usize, bool, bool);
/// A file of one diff, and its tokens; `None` when no grammar knows the file.
type Highlighted = HashMap<(Arc<str>, usize), Option<Arc<FileTokens>>>;

#[derive(Clone)]
pub struct DiffViewProps {
  pub diff: Arc<Diff>,
  /// Tells this diff's content apart from other diffs the view shows: highlighted files are kept per key.
  pub key: Arc<str>,
  /// Position of the file in the diff.
  pub index: usize,
  /// Where the change was read, at the head's end ("checkpointed at 7c1e9f2").
  pub source: Arc<str>,
}

impl PartialEq for DiffViewProps {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.diff, &other.diff)
      && self.key == other.key
      && self.index == other.index
      && self.source == other.source
  }
}

impl lurq::app::component::DevtoolsInspectable for DiffViewProps {}

pub struct DiffView {
  /// The view, and its width after the latest layout: rows are at least this wide.
  view: ElementRef,
  width: Signal<f32>,
  highlighted: Arc<Mutex<Highlighted>>,
  rows: Arc<Mutex<HashMap<RowsKey, Arc<Vec<DiffRow>>>>>,
}

impl Component for DiffView {
  type Props = DiffViewProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      view: ElementRef::new(),
      width: ctx.signal(0.0),
      highlighted: Arc::new(Mutex::new(HashMap::new())),
      rows: Arc::new(Mutex::new(HashMap::new())),
    }
  }

  /// Keeps the view's width after a layout that changed it, e.g. a resized window.
  fn after_layout(&self) {
    if !self.view.is_attached() {
      return;
    }
    let width = self.view.width();
    if (width - self.width.get_untracked()).abs() > WIDTH_EPSILON {
      self.width.set(width);
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DiffViewProps>().clone();
    let column = Column::new()
      .id("changes-diff")
      .ref_element(self.view.clone())
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .background(ExtraColor::CodeBackground.role());
    let Some(file) = props.diff.files().get(props.index) else {
      return column;
    };
    let tokens = self.tokens(ctx, &props);
    let rows = self.rows(&props, file, tokens.as_deref(), false);
    let view_width = self.width.get();
    let content = content_width(&rows);
    // Wider than the view: the horizontal scrollbar shows, below the last row.
    let rows = if content > view_width {
      self.rows(&props, file, tokens.as_deref(), true)
    } else {
      rows
    };
    let width = content.max(view_width);
    let list: Element = VirtualizedList::new_shared(ctx, rows)
      .list_key(format!("changes-diff-rows-{}-{}", props.key, props.index))
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .horizontal_scroll(true)
      .mount_keyed::<DiffRowView, _, _, _>(DiffRow::index, move |row| RowProps {
        row: row.clone(),
        width,
      });
    column
      .describe("highlighted", tokens.is_some().to_string())
      .child(head(ctx, file, &props.source))
      .child(list)
  }
}

impl DiffView {
  /// The highlighted tokens of the selected file once a background thread made them; a file of a diff
  /// is highlighted once while the view is open.
  fn tokens(&self, ctx: &mut Ctx, props: &DiffViewProps) -> Option<Arc<FileTokens>> {
    let key = (props.key.clone(), props.index);
    let cache = self.highlighted.clone();
    let deps = (props.key.clone(), props.index, props.diff.clone());
    let handle = ctx.future(deps, move |(diff_key, index, diff)| {
      let cache = cache.clone();
      async move {
        let key = (diff_key, index);
        if let Some(done) = cache.lock().get(&key) {
          return Ok((key, done.clone()));
        }
        let file = diff.files().get(index).cloned();
        let tokens = background::run(move || file.as_ref().and_then(highlight).map(Arc::new)).await?;
        cache.lock().insert(key.clone(), tokens.clone());
        Ok::<_, Message>((key, tokens))
      }
    });
    if let Some(done) = self.highlighted.lock().get(&key) {
      return done.clone();
    }
    // A pending refetch keeps the previous file's result; only this file's counts.
    match handle.state().get().data {
      Some((done, tokens)) if done == key => tokens,
      _ => None,
    }
  }

  /// The rows of `file`, with a `gap` for the horizontal scrollbar after them: the same list for the same
  /// file, tokens and gap, so the virtualised list sees an unchanged list between renders.
  fn rows(&self, props: &DiffViewProps, file: &FileDiff, tokens: Option<&FileTokens>, gap: bool) -> Arc<Vec<DiffRow>> {
    let key = (props.key.clone(), props.index, tokens.is_some(), gap);
    self
      .rows
      .lock()
      .entry(key)
      .or_insert_with(|| {
        let mut rows = rows(file, tokens);
        if gap {
          rows.push(DiffRow::ScrollbarGap { index: rows.len() });
        }
        Arc::new(rows)
      })
      .clone()
  }
}

/// The file's change, path and counts, and where its change was read (`rIRdw`).
fn head(ctx: &Ctx, file: &FileDiff, source: &str) -> Element {
  let (letter, color) = change_letter(ctx, file.change());
  Row::new()
    .id("changes-diff-head")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Lg))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&letter)
        .variant(CodeText::ChangeType.role())
        .color(color)
        .nowrap(),
    )
    .child(
      Text::new(file.path())
        .id("changes-diff-path")
        .variant(CodeText::DiffPath.role())
        .nowrap(),
    )
    .with_children(FileSummary::of(file).counts(ctx, ExtraTypography::MonoKey.role()))
    .child(Row::new().flex(1.0))
    .child(
      Text::new(source)
        .id("changes-diff-source")
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    )
    .into()
}
