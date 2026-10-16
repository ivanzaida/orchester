//! The selected file's diff (`Diff` `RsPIH` on `code_bg`): its head (`File head` `rIRdw`: the change's
//! letter, the path, the added and removed counts and where the change was read) and its rows in a
//! virtualised list that also scrolls sideways for long lines. The list is the file's own, by its source
//! and path, so reading the worktree again keeps its scroll position. The lines show plain at once and
//! highlighted when a background thread finished them; the tokens are kept by the file's content
//! (`cache`), so a file whose content did not change keeps its colors.

mod cache;
mod line;
mod rows;

use std::sync::Arc;

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
use orchester_core::diff::{Diff, FileChange, FileDiff};
use orchester_workspace::{
  background,
  highlight::{FileTokens, highlight},
  messages::Message,
};
use parking_lot::Mutex;

use self::{
  cache::{ContentKey, Highlights, ShownRows},
  line::{DiffRowView, RowProps},
  rows::DiffRow,
};
use super::files::{FileSummary, change_letter};
use crate::{
  navigation::Navigation,
  routes::Route,
  theme::{CodeText, ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// A diff, by its key, and a file's position in it.
type DiffAt = (Arc<str>, usize);

/// Height of the file head (`rIRdw`).
const HEAD_HEIGHT: f32 = 40.0;
/// Width changes below this are layout noise, not a resize.
const WIDTH_EPSILON: f32 = 0.5;

#[derive(Clone)]
pub struct DiffViewProps {
  pub diff: Arc<Diff>,
  /// Tells this diff's content apart from other diffs the view shows.
  pub key: Arc<str>,
  /// Position of the file in the diff.
  pub index: usize,
  /// Which file of the list it is, whatever its content: its source and path.
  pub file: Arc<str>,
  /// Where the change was read, at the head's end ("checkpointed at 7c1e9f2").
  pub source: Arc<str>,
  /// Where *Open file* takes the file: the code viewer at the version the change shows.
  pub open: Route,
}

impl PartialEq for DiffViewProps {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.diff, &other.diff)
      && self.key == other.key
      && self.index == other.index
      && self.file == other.file
      && self.source == other.source
      && self.open == other.open
  }
}

impl lurq::app::component::DevtoolsInspectable for DiffViewProps {}

pub struct DiffView {
  /// The view, and its width after the latest layout: rows are at least this wide.
  view: ElementRef,
  width: Signal<f32>,
  /// The content of the file shown, for the diff and position it was worked out of.
  content: Mutex<Option<(DiffAt, ContentKey)>>,
  highlights: Arc<Mutex<Highlights>>,
  shown: Mutex<Option<ShownRows>>,
}

impl Component for DiffView {
  type Props = DiffViewProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      view: ElementRef::new(),
      width: ctx.signal(0.0),
      content: Mutex::new(None),
      highlights: Arc::new(Mutex::new(Highlights::default())),
      shown: Mutex::new(None),
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
    let content = self.content(&props, file);
    let tokens = self.tokens(ctx, &props, &content);
    let shown = {
      let mut kept = self.shown.lock();
      let shown = ShownRows::of(kept.as_ref(), &content, file, tokens.as_deref());
      *kept = Some(shown.clone());
      shown
    };
    let width = shown.width.max(self.width.get());
    let list: Element = VirtualizedList::new_shared(ctx, shown.rows)
      .list_key(format!("changes-diff-rows-{}", props.file))
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .horizontal_scroll(true)
      .mount_keyed::<DiffRowView, _, _, _>(DiffRow::index, move |row| RowProps {
        row: row.clone(),
        width,
      });
    column
      .describe("highlighted", tokens.is_some().to_string())
      .child(head(ctx, file, (&props.source, &props.open)))
      .child(list)
  }
}

impl DiffView {
  /// The content of `file`, worked out once per diff and position.
  fn content(&self, props: &DiffViewProps, file: &FileDiff) -> ContentKey {
    let at = (props.key.clone(), props.index);
    let mut kept = self.content.lock();
    if let Some((kept_at, content)) = kept.as_ref()
      && *kept_at == at
      && content.file == props.file
    {
      return content.clone();
    }
    let content = ContentKey::of(props.file.clone(), file);
    *kept = Some((at, content.clone()));
    content
  }

  /// The highlighted tokens of the file once a background thread made them; a content is highlighted once
  /// while it stays among the files shown last.
  fn tokens(&self, ctx: &mut Ctx, props: &DiffViewProps, content: &ContentKey) -> Option<Arc<FileTokens>> {
    let highlights = self.highlights.clone();
    let deps = (content.clone(), props.diff.clone(), props.index);
    let handle = ctx.future(deps, move |(content, diff, index)| {
      let highlights = highlights.clone();
      async move {
        if let Some(done) = highlights.lock().get(&content) {
          return Ok((content, done));
        }
        let file = diff.files().get(index).cloned();
        let tokens = background::run(move || file.as_ref().and_then(highlight).map(Arc::new)).await?;
        highlights.lock().insert(content.clone(), tokens.clone());
        Ok::<_, Message>((content, tokens))
      }
    });
    if let Some(done) = self.highlights.lock().get(content) {
      return done;
    }
    // A pending refetch keeps the previous file's result; only this content's counts.
    match handle.state().get().data {
      Some((done, tokens)) if done == *content => tokens,
      _ => None,
    }
  }
}

/// The file's change, path and counts, where its change was read (`rIRdw`), and *Open file*, which opens the
/// file in the code viewer in a new tab.
fn head(ctx: &mut Ctx, file: &FileDiff, (source, open): (&str, &Route)) -> Element {
  let navigation = Navigation::of(ctx);
  let route = open.clone();
  let open = ActionButton::new("changes-open-file", ButtonKind::Ghost, ctx.t("changes.diff.openFile"))
    .icon(Icon::SquareArrowOutUpRight)
    .on_click(move || {
      navigation.open_tab();
      navigation.replace(route.clone());
    });
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
    .with_children(moved_from(ctx, file))
    .with_children(FileSummary::of(file).counts(ctx, ExtraTypography::MonoKey.role()))
    .child(Row::new().flex(1.0))
    .child(
      Text::new(source)
        .id("changes-diff-source")
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    )
    .child(open)
    .into()
}

/// Where a renamed or copied file came from ("from old/path"), after its path.
fn moved_from(ctx: &Ctx, file: &FileDiff) -> Option<Element> {
  let (FileChange::Renamed { .. } | FileChange::Copied { .. }) = file.change() else {
    return None;
  };
  let from = file.old_path()?;
  Some(
    Text::new(&ctx.t_args("changes.diff.renamedFrom", [("path", from)]))
      .id("changes-diff-from")
      .variant(ExtraTypography::Note.role())
      .nowrap()
      .into(),
  )
}
