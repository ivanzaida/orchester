//! The code diff of the selected file (`rz27k`): the diff toolbar (`SrS47`) with the compared
//! revisions and the unified mode, the file header (`nJCAp`), and the file's rows in a
//! virtualised list that also scrolls sideways for long lines. The file's lines show plain at
//! once and highlighted when a background thread finished them; highlighted files are kept per
//! head revision while the screen is open, so going back to a file does not highlight it again.

use std::{collections::HashMap, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text, VirtualizedList},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  pull_request::diff::{FileChange, FileDiff},
  revision::Revision,
};
use parking_lot::Mutex;

use super::{
  heading::SHORT_REVISION,
  line::{DiffRowView, RowProps, content_width},
  rows::{DiffRow, rows},
};
use crate::{
  background,
  messages::Message,
  pull_requests::{
    detail::ReadDiff,
    highlight::{FileTokens, highlight},
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{ValueTone, value_box_sized},
    icon::Icon,
    status_label::status_label,
  },
};

/// Toolbar height, the compared revisions' width and height, and the Viewed button's height
/// (`SrS47`, `e63KPo`, `q2AN6`).
const TOOLBAR_HEIGHT: f32 = 54.0;
const RANGE_WIDTH: f32 = 252.0;
const RANGE_HEIGHT: f32 = 32.0;
const VIEWED_HEIGHT: f32 = 30.0;
/// Width changes below this are layout noise, not a resize.
const WIDTH_EPSILON: f32 = 0.5;

/// The rows of one file of one head: plain, or highlighted; with room for a horizontal
/// scrollbar under them, or not.
type RowsKey = (Revision, usize, bool, bool);
/// A file of one head, and its tokens; `None` when no grammar knows the file.
type Highlighted = HashMap<(Revision, usize), Option<Arc<FileTokens>>>;

#[derive(Clone)]
pub struct DiffPaneProps {
  pub diff: Arc<ReadDiff>,
  pub index: usize,
  /// The base branch the diff starts from, when the pull request is known.
  pub base_branch: Option<Arc<str>>,
}

impl PartialEq for DiffPaneProps {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.diff, &other.diff) && self.index == other.index && self.base_branch == other.base_branch
  }
}

impl lurq::app::component::DevtoolsInspectable for DiffPaneProps {}

pub struct DiffPane {
  /// The pane, and its width after the latest layout: rows are at least this wide.
  pane: ElementRef,
  width: Signal<f32>,
  highlighted: Arc<Mutex<Highlighted>>,
  rows: Arc<Mutex<HashMap<RowsKey, Arc<Vec<DiffRow>>>>>,
}

impl Component for DiffPane {
  type Props = DiffPaneProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      pane: ElementRef::new(),
      width: ctx.signal(0.0),
      highlighted: Arc::new(Mutex::new(HashMap::new())),
      rows: Arc::new(Mutex::new(HashMap::new())),
    }
  }

  /// Keeps the pane's width after a layout that changed it, e.g. a resized window.
  fn after_layout(&self) {
    if !self.pane.is_attached() {
      return;
    }
    let width = self.pane.width();
    if (width - self.width.get_untracked()).abs() > WIDTH_EPSILON {
      self.width.set(width);
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DiffPaneProps>().clone();
    let Some(file) = props.diff.diff.files().get(props.index) else {
      return Column::new().flex(1.0);
    };
    let tokens = self.tokens(ctx, &props);
    let rows = self.rows(&props, file, tokens.as_deref(), false);
    let pane_width = self.width.get();
    let content = content_width(&rows);
    // Wider than the pane: the horizontal scrollbar shows, below the last row.
    let rows = if content > pane_width {
      self.rows(&props, file, tokens.as_deref(), true)
    } else {
      rows
    };
    let width = content.max(pane_width);
    let list: Element = VirtualizedList::new_shared(ctx, rows)
      .list_key(format!("pull-request-diff-{}", props.index))
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .horizontal_scroll(true)
      .mount_keyed::<DiffRowView, _, _, _>(DiffRow::index, move |row| RowProps {
        row: row.clone(),
        width,
      });
    Column::new()
      .id("pull-request-diff")
      .ref_element(self.pane.clone())
      .describe("highlighted", tokens.is_some().to_string())
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .child(toolbar(ctx, &props))
      .child(file_header(ctx, file))
      .child(list)
  }
}

impl DiffPane {
  /// The highlighted tokens of the selected file once a background thread made them; a file of
  /// a head is highlighted once while the screen is open.
  fn tokens(&self, ctx: &mut Ctx, props: &DiffPaneProps) -> Option<Arc<FileTokens>> {
    let key = (props.diff.head.clone(), props.index);
    let cache = self.highlighted.clone();
    let handle = ctx.future((props.diff.clone(), props.index), move |(diff, index)| {
      let cache = cache.clone();
      async move {
        let key = (diff.head.clone(), index);
        if let Some(done) = cache.lock().get(&key) {
          return Ok((key, done.clone()));
        }
        let file = diff.diff.files().get(index).cloned();
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

  /// The rows of `file`, with a `gap` for the horizontal scrollbar after them: the same list for
  /// the same file, tokens and gap, so the virtualised list sees an unchanged list between
  /// renders.
  fn rows(&self, props: &DiffPaneProps, file: &FileDiff, tokens: Option<&FileTokens>, gap: bool) -> Arc<Vec<DiffRow>> {
    let key = (props.diff.head.clone(), props.index, tokens.is_some(), gap);
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

/// The compared revisions, the unified mode and Viewed (`SrS47`). Other ranges and Viewed
/// markers do not exist yet: both show as unavailable.
fn toolbar(ctx: &Ctx, props: &DiffPaneProps) -> Element {
  let revision = &props.diff.head.as_str()[..SHORT_REVISION];
  let range = match &props.base_branch {
    Some(base) => ctx.t_args("pullRequests.diff.range", [("base", &**base), ("revision", revision)]),
    None => ctx.t_args("pullRequests.diff.rangeHead", [("revision", revision)]),
  };
  let range = Column::new().width(Dimension::Px(RANGE_WIDTH)).child(value_box_sized(
    ctx,
    "pull-request-range",
    &range,
    ValueTone::Unavailable,
    None,
    Dimension::Px(RANGE_HEIGHT),
  ));
  let viewed = ActionButton::new(ctx.t("pullRequests.diff.viewed"))
    .icon(Icon::Square)
    .id("pull-request-viewed")
    .height(Dimension::Px(VIEWED_HEIGHT))
    .render(ctx);
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(TOOLBAR_HEIGHT))
    .padding_horizontal(SpacingSize::Md)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(range)
    .child(Spacer::new().flex(1.0))
    .child(status_label(Icon::AlignLeft, &ctx.t("pullRequests.diff.unified")))
    .child(viewed)
    .into()
}

/// The file's path and what happened to it (`nJCAp`).
fn file_header(ctx: &Ctx, file: &FileDiff) -> Element {
  let (additions, deletions) = (file.additions().to_string(), file.deletions().to_string());
  let sizes = [("additions", additions.as_str()), ("deletions", deletions.as_str())];
  let meta = match (file.change(), file.old_path()) {
    (FileChange::Renamed { .. }, Some(from)) => {
      ctx.t_args("pullRequests.diff.meta.renamed", [sizes[0], sizes[1], ("from", from)])
    }
    (FileChange::Copied { .. }, Some(from)) => {
      ctx.t_args("pullRequests.diff.meta.copied", [sizes[0], sizes[1], ("from", from)])
    }
    (FileChange::Added, _) => ctx.t_args("pullRequests.diff.meta.added", sizes),
    (FileChange::Deleted, _) => ctx.t_args("pullRequests.diff.meta.deleted", sizes),
    _ => ctx.t_args("pullRequests.diff.meta.modified", sizes),
  };
  Column::new()
    .id("pull-request-file-header")
    .width(Dimension::Pct(100.0))
    .padding_vertical(ExtraSpacing::Divider.role())
    .padding_horizontal(ExtraSpacing::Step.role())
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(file.path())
        .variant(ExtraTypography::DiffPath.role())
        .nowrap(),
    )
    .child(Text::new(&meta).variant(ExtraTypography::Footnote.role()).nowrap())
    .into()
}
