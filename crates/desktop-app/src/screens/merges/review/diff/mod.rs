//! The run's whole diff on the review page (`Diff` `l7sLk` in 09i, in a `border` stroke, radius 6, on `code_bg`): every
//! file's head and, unless folded, its hunks and lines with the threads, folds and the comment box under them, in one
//! virtualised list that also scrolls sideways for long lines, so a large diff scrolls without laying out what is off
//! screen. Lines show plain at once and highlighted once a background thread finished every file. Choosing a file in
//! the list scrolls to its head.

mod head;
mod line;
mod place;
mod row;
mod rows;

use std::{
  collections::{BTreeMap, BTreeSet, HashMap},
  sync::Arc,
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Column, VirtualizedList},
  core::{ElementRef, Signal},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::diff::Diff;
use orchester_workspace::{
  background,
  highlight::{FileTokens, highlight},
  messages::Message,
};
use parking_lot::Mutex;

pub(super) use self::{
  place::{COMPOSER, outdated_key, resolved_key, thread_key},
  rows::ReviewRow,
};
use self::{
  place::{FilePlaces, places},
  row::{RowProps, RowView},
  rows::made,
};
use super::Env;
use crate::theme::ExtraColor;

/// Width changes below this are layout noise, not a resize.
const WIDTH_EPSILON: f32 = 0.5;

/// The tokens of every file of a diff, by position; `None` for a file no grammar knows.
type AllTokens = Arc<Vec<Option<Arc<FileTokens>>>>;

#[derive(Clone)]
pub(super) struct DiffProps {
  pub env: Arc<Env>,
  pub diff: Arc<Diff>,
  /// Tells this diff's content apart from others.
  pub key: Arc<str>,
}

impl PartialEq for DiffProps {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.diff, &other.diff) && self.key == other.key && self.env.version == other.env.version
  }
}

impl DevtoolsInspectable for DiffProps {}

/// What the rows were made of.
#[derive(Clone, PartialEq)]
struct MadeOf {
  key: Arc<str>,
  highlighted: bool,
  places: HashMap<Arc<str>, FilePlaces>,
  folded: BTreeMap<Arc<str>, bool>,
  viewed: Arc<BTreeSet<String>>,
}

pub(super) struct ReviewDiff {
  view: ElementRef,
  width: Signal<f32>,
  made: Mutex<Option<(MadeOf, rows::Made)>>,
}

impl Component for ReviewDiff {
  type Props = DiffProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      view: ElementRef::new(),
      width: ctx.signal(0.0),
      made: Mutex::new(None),
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
    let props = ctx.props::<DiffProps>().clone();
    let tokens = tokens(ctx, &props);
    let env = props.env.clone();
    let state = &env.state;
    let composing = state.composing.get().0;
    let opened = state.opened.get().0;
    let folded = state.folded.get().0;
    let places = places(&props.diff, (&env.review, &env.head), composing.as_ref(), &opened);
    let made_of = MadeOf {
      key: props.key.clone(),
      highlighted: tokens.is_some(),
      places,
      folded,
      viewed: env.viewed.clone(),
    };
    let (rows, content) = self.rows(&props.diff, tokens.as_deref().map(Vec::as_slice), made_of);
    let view_width = self.width.get();
    let width = content.max(view_width);
    let reveal = state.chosen.get().0.and_then(|path| {
      let at = props.diff.files().iter().position(|file| *file.path() == *path)?;
      rows.iter().find_map(|row| match row {
        ReviewRow::Head { index, file } if *file == at => Some(index.to_string()),
        _ => None,
      })
    });
    let release = state.clone();
    let list = VirtualizedList::new_shared(ctx, rows)
      .list_key(format!("review-diff-rows-{}", env.page.run.run))
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .horizontal_scroll(true)
      .reveal_key(reveal)
      .mount_keyed::<RowView, _, _, _>(ReviewRow::index, move |row| RowProps {
        row: row.clone(),
        width,
        view_width,
        env: env.clone(),
      });
    Column::new()
      .id("review-diff")
      .ref_element(self.view.clone())
      .describe("highlighted", tokens.is_some().to_string())
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .background(ExtraColor::CodeBackground.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .on_mouse_up(move |_| release.release())
      .child(list)
  }
}

impl ReviewDiff {
  /// The rows as `made_of` makes them, made again only when that changed, and the width the widest line needs.
  fn rows(
    &self,
    diff: &Diff,
    tokens: Option<&[Option<Arc<FileTokens>>]>,
    made_of: MadeOf,
  ) -> (Arc<Vec<ReviewRow>>, f32) {
    let mut kept = self.made.lock();
    if let Some((was, made)) = kept.as_ref()
      && *was == made_of
    {
      return (made.rows.clone(), made.width);
    }
    let folded = |file: &orchester_core::diff::FileDiff| {
      let path = file.path();
      made_of
        .folded
        .get(path)
        .copied()
        .unwrap_or_else(|| made_of.viewed.contains(path))
    };
    let made = made(diff, tokens, &made_of.places, &folded);
    let shown = (made.rows.clone(), made.width);
    *kept = Some((made_of, made));
    shown
  }
}

/// The tokens of every file of the diff once a background thread made them.
fn tokens(ctx: &mut Ctx, props: &DiffProps) -> Option<AllTokens> {
  let handle = ctx.future((props.key.clone(), props.diff.clone()), |(key, diff)| async move {
    let tokens = background::run(move || {
      diff
        .files()
        .iter()
        .map(|file| highlight(file).map(Arc::new))
        .collect::<Vec<_>>()
    })
    .await?;
    Ok::<_, Message>((key, Arc::new(tokens)))
  });
  // A pending refetch keeps the previous diff's result; only this diff's counts.
  match handle.state().get().data {
    Some((key, tokens)) if key == props.key => Some(tokens),
    _ => None,
  }
}
