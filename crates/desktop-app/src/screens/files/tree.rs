//! The folder tree (`Tree` `ceNFg`, 280 wide, a `border` line at its end): `FILES` with *Collapse all*, then
//! the version's folders and files, virtualised. A row (`Folder · crates` `qlBNS`, `File · budget.rs`
//! `G6DxKg`) is 24 high, radius 6, 8 at its sides and 14 further in per level: a 12 `text_muted` chevron (an
//! empty place for a file), a 14 icon and the name in 12 `text_secondary`; the file shown is on `hover`, its
//! icon and name `text_primary` in 12/500 (`MeV4Y`). A folder opens or closes on a press, a file opens.
//! The workspace lays the rows out ([`FileList::rows`]).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, VirtualizedList},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::files::{EntryKind, FileList, FilePath, TreeRow};
use parking_lot::Mutex;

use super::{OpenFolders, Page, reads::Shown};
use crate::{
  navigation::{Navigation, press::OpenIn},
  routes::{FilesView, ProjectPage, Route},
  theme::{CodeText, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, file_icon, icon},
    icon_button::IconButton,
    message::message_text,
    pressable::{opening, pressable},
    state::loading,
  },
};

/// The tree's width (`ceNFg`), a row's height (`qlBNS`) and how much further in each level is.
pub const TREE_WIDTH: f32 = 280.0;
const ROW_HEIGHT: f32 = 24.0;
const LEVEL_INDENT: f32 = 14.0;
/// The room of a chevron, kept empty beside a file (`No chevron` `i2EHoK`).
const CHEVRON_ROOM: f32 = 12.0;

/// The tree beside the file: its head and its rows, or that they are being read or cannot be.
pub fn panel(ctx: &mut Ctx, page: &Page<'_>, list: &Shown<FileList>) -> Element {
  let open = page.view.open.clone();
  let head = Row::new()
    .id("files-tree-head")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Md)
        .bottom(ExtraSpacing::Snug.role())
        .left(ExtraSpacing::Loose.role())
        .right(ExtraSpacing::Loose.role()),
    )
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("files.tree.heading"))
        .variant(CodeText::GroupHeading.role())
        .nowrap(),
    )
    .child(Row::new().flex(1.0))
    .child(
      IconButton::new(
        "files-tree-collapse",
        Icon::ChevronsDownUp,
        ctx.t("files.tree.collapse"),
      )
      .compact()
      .on_click(move || open.set(OpenFolders::default()))
      .render(ctx),
    );
  // The rows reach the panel's right edge, where the list draws its scrollbar, and leave it a gutter.
  let gutter = |element: Element| -> Element {
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().right(SpacingSize::Sm))
      .child(element)
      .into()
  };
  let content: Element = match list {
    Shown::Reading => gutter(loading("files-tree-loading", &ctx.t("files.reading")).into()),
    Shown::Failed(failure) => gutter(
      Text::new(&message_text(ctx, &failure.message()))
        .id("files-tree-error")
        .variant(TypographyStyle::Caption)
        .width(Dimension::Pct(100.0))
        .into(),
    ),
    Shown::Read(list) if list.is_empty() => gutter(
      Text::new(&ctx.t("files.tree.empty"))
        .id("files-tree-empty")
        .variant(TypographyStyle::Caption)
        .into(),
    ),
    Shown::Read(list) => {
      let choose = chooser(ctx, page);
      ctx.mount_keyed::<Tree>(
        "files-tree",
        TreeProps {
          list: list.clone(),
          open: page.view.open.clone(),
          shown: page.file().map(FilePath::shared),
          choose,
        },
      )
    }
  };
  Column::new()
    .id("files-tree-panel")
    .width(Dimension::Px(TREE_WIDTH))
    .height(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Xs)
        .bottom(SpacingSize::Xs)
        .left(SpacingSize::Sm),
    )
    .border_right(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(gutter(head.into()))
    .child(content)
    .into()
}

type Choose = Arc<dyn Fn(&TreeRow, OpenIn) + Send + Sync>;

/// A folder opens or closes, a file opens at the version shown where a press asks (`navigation::link`).
fn chooser(ctx: &mut Ctx, page: &Page<'_>) -> Choose {
  let open = page.view.open.clone();
  let navigation = Navigation::of(ctx);
  let (project, at) = (page.project.open.project.id(), page.at().clone());
  Arc::new(move |row: &TreeRow, place: OpenIn| match row.kind {
    EntryKind::Folder { .. } => open.set(open.get_untracked().toggled(&row.path)),
    EntryKind::File => {
      let view = FilesView::new(Some(&row.path), at.clone());
      navigation.open(Route::Project(project, ProjectPage::Files(view)), place);
    }
  })
}

#[derive(Clone)]
struct TreeProps {
  list: FileList,
  open: Signal<OpenFolders>,
  shown: Option<Arc<str>>,
  choose: Choose,
}

impl PartialEq for TreeProps {
  fn eq(&self, other: &Self) -> bool {
    self.list == other.list && self.open.id() == other.open.id() && self.shown == other.shown
  }
}

impl DevtoolsInspectable for TreeProps {}

/// One row as the list mounts it.
#[derive(Clone, PartialEq)]
struct Item {
  row: TreeRow,
  shown: bool,
}

impl DevtoolsInspectable for Item {}

/// What rows are laid out for: the list, the folders open and the file shown.
type RowsFor = (FileList, OpenFolders, Option<Arc<str>>);

struct Tree {
  /// The rows, for the folders open and the file shown they were laid out for.
  rows: Mutex<Option<(RowsFor, Arc<Vec<Item>>)>>,
}

impl Component for Tree {
  type Props = TreeProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self { rows: Mutex::new(None) }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TreeProps>().clone();
    let open = props.open.get();
    let items = self.items(&props, open);
    let choose = props.choose.clone();
    VirtualizedList::new_shared(ctx, items)
      .list_key("files-tree-rows")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .reveal_key(props.shown.as_deref().map(str::to_owned))
      .mount_keyed::<RowView, _, _, _>(
        |item: &Item| item.row.path.clone(),
        move |item| RowProps {
          item: item.clone(),
          choose: choose.clone(),
        },
      )
  }
}

impl Tree {
  fn items(&self, props: &TreeProps, open: OpenFolders) -> Arc<Vec<Item>> {
    let key = (props.list.clone(), open, props.shown.clone());
    let mut kept = self.rows.lock();
    if let Some((made_for, items)) = kept.as_ref()
      && *made_for == key
    {
      return items.clone();
    }
    let items: Arc<Vec<Item>> = Arc::new(
      props
        .list
        .rows(key.1.set())
        .into_iter()
        .map(|row| Item {
          shown: Some(&row.path) == key.2.as_ref(),
          row,
        })
        .collect(),
    );
    *kept = Some((key, items.clone()));
    items
  }
}

#[derive(Clone)]
struct RowProps {
  item: Item,
  choose: Choose,
}

impl PartialEq for RowProps {
  fn eq(&self, other: &Self) -> bool {
    self.item == other.item
  }
}

impl DevtoolsInspectable for RowProps {}

struct RowView;

impl Component for RowView {
  type Props = RowProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let RowProps { item, choose } = ctx.props::<RowProps>().clone();
    let Item { row, shown } = item;
    let (chevron, glyph) = match row.kind {
      EntryKind::Folder { open: true } => (Some(Icon::ChevronDown), Icon::FolderOpen),
      EntryKind::Folder { open: false } => (Some(Icon::ChevronRight), Icon::Folder),
      EntryKind::File => (None, file_icon(&row.path)),
    };
    let (glyph_color, name) = if shown {
      (
        PaletteColor::TextPrimary,
        Text::new(&row.name).variant(TypographyStyle::Label),
      )
    } else if chevron.is_some() {
      (
        PaletteColor::TextSecondary,
        Text::new(&row.name).variant(ExtraTypography::Property.role()),
      )
    } else {
      (
        PaletteColor::TextMuted,
        Text::new(&row.name).variant(ExtraTypography::Property.role()),
      )
    };
    // The indent stands before the chevron with the row's gap after it, which it takes back.
    #[allow(
      clippy::cast_precision_loss,
      reason = "a tree is far fewer levels deep than f32 counts exactly"
    )]
    let indent = (row.depth > 0).then(|| {
      Row::new().width(Dimension::Px(
        row.depth as f32 * LEVEL_INDENT - ExtraSpacing::Snug.value(),
      ))
    });
    let chevron: Element = match chevron {
      Some(chevron) => icon(chevron, IconSize::Sm, PaletteColor::TextMuted).into(),
      None => Row::new().width(Dimension::Px(CHEVRON_ROOM)).into(),
    };
    let kind = match row.kind {
      EntryKind::Folder { open } => {
        if open {
          "open-folder"
        } else {
          "folder"
        }
      }
      EntryKind::File => "file",
    };
    let target = row.clone();
    // A file is a place of its own; a folder opens or closes in the tree.
    let line = match row.kind {
      EntryKind::File => opening(Arc::new(move |place| choose(&target, place))),
      EntryKind::Folder { .. } => pressable(move || choose(&target, OpenIn::ActiveTab)),
    };
    let line = line
      .id(format!("files-tree:{}", row.path))
      .describe("path", row.path.to_string())
      .describe("kind", kind)
      .describe("state", if shown { "selected" } else { "enabled" })
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ROW_HEIGHT))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .with_children(indent)
      .child(chevron)
      .child(icon(glyph, IconSize::Md, glyph_color))
      .child(name.nowrap());
    let line = if shown {
      line.background(ExtraColor::Hover.role())
    } else {
      line.hovered_style(Style::new().background(ExtraColor::Hover.role()))
    };
    // The scrollbar's gutter: the list draws its bar beside the rows, never over the chosen one.
    Row::new()
      .id(format!("files-tree-gutter:{}", row.path))
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().right(SpacingSize::Sm))
      .child(line)
  }
}
