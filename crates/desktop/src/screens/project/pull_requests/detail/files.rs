//! The changed file list (`h8PZJa`): its heading with the count, Find a file (`VkyIC`, the one
//! control of the screen that takes focus), and the files grouped under their directories
//! (`WD8U6`), each a changed file (`j9NDM`) with its status and size. Virtualised, since a pull
//! request may change hundreds of files. Its own component, so typing a search re-renders only
//! the list. The design's viewed count (`AYNQU`) belongs to Viewed markers, which do not exist yet.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow, VirtualizedList},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::diff::{FileChange, FileDiff};

use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{
    field::styled_input,
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
  },
};

/// Width of the file list and heights of its search field and file rows (`h8PZJa`, `VkyIC`,
/// `j9NDM`).
pub const LIST_WIDTH: f32 = 192.0;
const SEARCH_HEIGHT: f32 = 32.0;
const FILE_HEIGHT: f32 = 52.0;

/// A changed file as the list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
  /// Position in the diff.
  pub index: usize,
  pub directory: Arc<str>,
  pub name: Arc<str>,
  pub path: Arc<str>,
  pub status: &'static str,
  pub additions: u32,
  pub deletions: u32,
  pub binary: bool,
}

impl FileEntry {
  pub fn of(index: usize, file: &FileDiff) -> Self {
    let path = file.path();
    let (directory, name) = path.rsplit_once('/').unwrap_or(("", path));
    Self {
      index,
      directory: directory.into(),
      name: name.into(),
      path: path.into(),
      status: status_key(file.change()),
      additions: file.additions(),
      deletions: file.deletions(),
      binary: file.is_binary(),
    }
  }
}

/// The letter of a file's change (`PUtxL`: "M", "A").
const fn status_key(change: &FileChange) -> &'static str {
  match change {
    FileChange::Added => "pullRequests.files.status.added",
    FileChange::Deleted => "pullRequests.files.status.deleted",
    FileChange::Modified => "pullRequests.files.status.modified",
    FileChange::Renamed { .. } => "pullRequests.files.status.renamed",
    FileChange::Copied { .. } => "pullRequests.files.status.copied",
  }
}

#[derive(Clone)]
pub struct FileListProps {
  pub files: Arc<Vec<FileEntry>>,
  pub selected: usize,
  pub select: Signal<usize>,
}

impl PartialEq for FileListProps {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.files, &other.files) && self.selected == other.selected
  }
}

impl lurq::app::component::DevtoolsInspectable for FileListProps {}

pub struct FileList {
  query: Signal<String>,
  search_focused: Signal<bool>,
}

impl Component for FileList {
  type Props = FileListProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      query: ctx.signal(String::new()),
      search_focused: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<FileListProps>().clone();
    let count = props.files.len().to_string();
    let query = self.query.get().trim().to_lowercase();
    let items = Arc::new(items(&props.files, props.selected, &query));
    let select = props.select;
    let list: Element = if items.is_empty() {
      Text::new(&ctx.t("pullRequests.files.noMatch"))
        .variant(TypographyStyle::Caption)
        .into()
    } else {
      let scrollbar = gutter_scrollbar(&ctx.theme().scrollbar());
      VirtualizedList::new_shared(ctx, items)
        .list_key("pull-request-files")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .scrollbar(scrollbar)
        .mount_keyed::<FileRow, _, _, _>(FileItem::key, move |item| FileRowProps {
          item: item.clone(),
          select: select.clone(),
        })
    };
    Column::new()
      .id("pull-request-files")
      .width(Dimension::Px(LIST_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding_vertical(ExtraSpacing::Step.role())
      // The right side's padding is the list's scrollbar gutter; the heading and search keep it.
      .padding_left(ExtraSpacing::Stack.role())
      .spacing(SpacingSize::Sm)
      .background(PaletteColor::SurfacePanel)
      .border_right(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding_right(ExtraSpacing::Stack.role())
          .spacing(SpacingSize::Sm)
          .child(
            Text::new(&ctx.t_args("pullRequests.files.heading", [("count", count.as_str())]))
              .variant(ExtraTypography::FileListHeading.role())
              .nowrap(),
          )
          .child(self.search(ctx)),
      )
      .child(list)
  }
}

impl FileList {
  /// Find a file (`VkyIC`).
  fn search(&self, ctx: &Ctx) -> Element {
    let input = ctx.theme().form().input.clone();
    let border = if self.search_focused.get() {
      Border::inside(BorderSize::Md, input.border_focus.clone())
    } else {
      Border::inside(BorderSize::Sm, input.border.clone())
    };
    let (focus, blur) = (self.search_focused.clone(), self.search_focused.clone());
    let field = styled_input(ctx, self.query.clone(), TypographyStyle::FieldLabel)
      .id("pull-request-file-search")
      .single_line()
      .flex(1.0)
      .placeholder(&ctx.t("pullRequests.files.search"))
      .on_focus(move || focus.set(true))
      .on_blur(move || blur.set(false));
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(SEARCH_HEIGHT))
      .padding_custom(input.padding)
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .background(input.background)
      .border(border)
      .rounded(input.radius)
      .child(field)
      .child(icon(Icon::Search, IconSize::Dismiss, PaletteColor::TextSecondary))
      .into()
  }
}

/// A directory label or a file of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
enum FileItem {
  Directory(Arc<str>),
  File { entry: FileEntry, selected: bool },
}

impl FileItem {
  fn key(&self) -> String {
    match self {
      Self::Directory(directory) => format!("directory:{directory}"),
      Self::File { entry, .. } => format!("file:{}", entry.index),
    }
  }
}

/// The files whose path contains `query`, each group of files of one directory after its label.
/// Files at the repository root have no label.
fn items(files: &[FileEntry], selected: usize, query: &str) -> Vec<FileItem> {
  let mut items = Vec::new();
  let mut directory: Option<&str> = None;
  for entry in files
    .iter()
    .filter(|entry| query.is_empty() || entry.path.to_lowercase().contains(query))
  {
    if directory != Some(&*entry.directory) {
      directory = Some(&entry.directory);
      if !entry.directory.is_empty() {
        items.push(FileItem::Directory(entry.directory.clone()));
      }
    }
    items.push(FileItem::File {
      entry: entry.clone(),
      selected: entry.index == selected,
    });
  }
  items
}

#[derive(Clone)]
struct FileRowProps {
  item: FileItem,
  select: Signal<usize>,
}

impl PartialEq for FileRowProps {
  fn eq(&self, other: &Self) -> bool {
    self.item == other.item
  }
}

impl lurq::app::component::DevtoolsInspectable for FileRowProps {}

struct FileRow;

impl Component for FileRow {
  type Props = FileRowProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let FileRowProps { item, select } = ctx.props::<FileRowProps>().clone();
    let content: Element = match item {
      FileItem::Directory(directory) => Row::new()
        .child(
          Text::new(&directory)
            .variant(TypographyStyle::Mono)
            .nowrap()
            .text_overflow(TextOverflow::Elipsis),
        )
        .into(),
      FileItem::File { entry, selected } => file_row(ctx, &entry, selected, select),
    };
    // The list's 8px gap (`h8PZJa`), inside each item so the virtualised list measures it.
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding_bottom(SpacingSize::Sm)
      .child(content)
  }
}

/// A changed file (`j9NDM`); the selected one on the hover fill (`QtCOv`).
fn file_row(ctx: &Ctx, entry: &FileEntry, selected: bool, select: Signal<usize>) -> Element {
  let status = ctx.t(entry.status);
  let summary = if entry.binary {
    ctx.t_args("pullRequests.files.summaryBinary", [("status", &*status)])
  } else {
    ctx.t_args(
      "pullRequests.files.summary",
      [
        ("status", &*status),
        ("additions", entry.additions.to_string().as_str()),
        ("deletions", entry.deletions.to_string().as_str()),
      ],
    )
  };
  let content = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(&entry.name)
        .variant(ExtraTypography::FileName.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(Text::new(&summary).variant(TypographyStyle::Mono).nowrap());
  let row = surface()
    .id(format!("pull-request-file:{}", entry.path))
    .describe("path", entry.path.to_string())
    .describe("selected", selected.to_string())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FILE_HEIGHT))
    .padding_vertical(ExtraSpacing::Inline.role())
    .padding_horizontal(ExtraSpacing::Stack.role())
    .rounded(RadiusSize::Sm)
    .child(content);
  if selected {
    return row.background(PaletteColor::AccentMuted).into();
  }
  let index = entry.index;
  pressable(row, move || select.set(index))
    .hovered_style(Style::new().background(PaletteColor::SurfaceRaised))
    .into()
}
