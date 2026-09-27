//! The changed file list (`h8PZJa`): its heading with the count, Find a file (`VkyIC`, a field
//! that takes focus), and the files grouped under their directories (`WD8U6`), each a changed file
//! (`j9NDM`) with its status and size. A list may hold several groups of files, each after its
//! heading (a run's checkpointed changes, then those not checkpointed). Virtualised, since a
//! change may hold hundreds of files. Its own component, so typing a search re-renders only the
//! list. The design's viewed count (`AYNQU`) belongs to Viewed markers, which do not exist yet.

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
  /// Position in the list, across its groups: what selecting the file selects.
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

/// Files under one heading. `key` names the group in element ids: a file row is `<key>:<path>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileGroup {
  pub key: &'static str,
  pub heading: Arc<str>,
  pub files: Vec<FileEntry>,
}

/// The letter of a file's change (`PUtxL`: "M", "A").
const fn status_key(change: &FileChange) -> &'static str {
  match change {
    FileChange::Added => "diff.files.status.added",
    FileChange::Deleted => "diff.files.status.deleted",
    FileChange::Modified => "diff.files.status.modified",
    FileChange::Renamed { .. } => "diff.files.status.renamed",
    FileChange::Copied { .. } => "diff.files.status.copied",
  }
}

/// Called with the list position of the file a person chose.
pub type SelectFile = Arc<dyn Fn(usize) + Send + Sync>;

#[derive(Clone)]
pub struct FileListProps {
  /// The first group's heading tops the list; later groups follow their files.
  pub groups: Arc<Vec<FileGroup>>,
  pub selected: usize,
  pub select: SelectFile,
}

impl PartialEq for FileListProps {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.groups, &other.groups) && self.selected == other.selected
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
    let heading = props
      .groups
      .first()
      .map(|group| group.heading.clone())
      .unwrap_or_default();
    let query = self.query.get().trim().to_lowercase();
    let items = Arc::new(items(&props.groups, props.selected, &query));
    let select = props.select;
    let list: Element = if items.is_empty() {
      Text::new(&ctx.t("diff.files.noMatch"))
        .variant(TypographyStyle::Caption)
        .into()
    } else {
      let scrollbar = gutter_scrollbar(&ctx.theme().scrollbar());
      VirtualizedList::new_shared(ctx, items)
        .list_key("diff-files")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .scrollbar(scrollbar)
        .mount_keyed::<FileRow, _, _, _>(FileItem::key, move |item| FileRowProps {
          item: item.clone(),
          select: select.clone(),
        })
    };
    Column::new()
      .id("diff-files")
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
          .child(group_heading(&heading))
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
      .id("diff-file-search")
      .single_line()
      .flex(1.0)
      .placeholder(&ctx.t("diff.files.search"))
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

/// "FILES  6" (`UwSNk`).
fn group_heading(heading: &str) -> Element {
  Text::new(heading)
    .variant(ExtraTypography::FileListHeading.role())
    .nowrap()
    .into()
}

/// A later group's heading, a directory label or a file of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
enum FileItem {
  Heading(Arc<str>),
  Directory(&'static str, Arc<str>),
  File {
    group: &'static str,
    entry: FileEntry,
    selected: bool,
  },
}

impl FileItem {
  fn key(&self) -> String {
    match self {
      Self::Heading(heading) => format!("heading:{heading}"),
      Self::Directory(group, directory) => format!("directory:{group}:{directory}"),
      Self::File { entry, .. } => format!("file:{}", entry.index),
    }
  }
}

/// The files whose path contains `query`, each group of files of one directory after its label,
/// and each group after the first under its heading. Files at the repository root have no label,
/// and a group without matching files has no heading.
fn items(groups: &[FileGroup], selected: usize, query: &str) -> Vec<FileItem> {
  let mut items = Vec::new();
  for (position, group) in groups.iter().enumerate() {
    let mut directory: Option<&str> = None;
    let matching = group
      .files
      .iter()
      .filter(|entry| query.is_empty() || entry.path.to_lowercase().contains(query));
    for entry in matching {
      if position > 0 && directory.is_none() {
        items.push(FileItem::Heading(group.heading.clone()));
      }
      if directory != Some(&*entry.directory) {
        directory = Some(&entry.directory);
        if !entry.directory.is_empty() {
          items.push(FileItem::Directory(group.key, entry.directory.clone()));
        }
      }
      items.push(FileItem::File {
        group: group.key,
        entry: entry.clone(),
        selected: entry.index == selected,
      });
    }
  }
  items
}

#[derive(Clone)]
struct FileRowProps {
  item: FileItem,
  select: SelectFile,
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
      // Set apart from the files above by the list's gap and a divider's worth of room.
      FileItem::Heading(heading) => Column::new()
        .width(Dimension::Pct(100.0))
        .padding_top(SpacingSize::Sm)
        .child(group_heading(&heading))
        .into(),
      FileItem::Directory(_, directory) => Row::new()
        .child(
          Text::new(&directory)
            .variant(TypographyStyle::Mono)
            .nowrap()
            .text_overflow(TextOverflow::Elipsis),
        )
        .into(),
      FileItem::File { group, entry, selected } => file_row(ctx, group, &entry, selected, select),
    };
    // The list's 8px gap (`h8PZJa`), inside each item so the virtualised list measures it.
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding_bottom(SpacingSize::Sm)
      .child(content)
  }
}

/// A changed file (`j9NDM`); the selected one on the hover fill (`QtCOv`).
fn file_row(ctx: &Ctx, group: &str, entry: &FileEntry, selected: bool, select: SelectFile) -> Element {
  let status = ctx.t(entry.status);
  let summary = if entry.binary {
    ctx.t_args("diff.files.summaryBinary", [("status", &*status)])
  } else {
    ctx.t_args(
      "diff.files.summary",
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
    .id(format!("{group}:{}", entry.path))
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
  pressable(row, move || select(index))
    .hovered_style(Style::new().background(PaletteColor::SurfaceRaised))
    .into()
}
