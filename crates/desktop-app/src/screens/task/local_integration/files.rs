//! The changed paths of the checkout, one row per reviewed snapshot, in a radius 6 `border` frame under its title
//! in 13 `text_primary` (`Local changes` `Q5DPgQ`, `Commit selection` `q43ZSk`, `Preservation selection`
//! `CCvnb`). A group's label is 11.5 `text_muted` on `surface_raised`, 6 over and under and 10 at its sides
//! (`Staged heading` `MJbQq`). A row is 8 over and under and 10 at its sides over a `border` line: its checkbox,
//! then 8 on its change's letter and its path, which wraps where the row ends, and 2 under them what the
//! snapshot holds in 11.5 `text_muted` (`juh4k`). In the review and the commit review a press on the path shows
//! its snapshot's diff; the commit and preservation reviews list only the selected snapshots, which cannot change
//! there.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, VirtualizedList},
  core::Signal,
  layout::{Alignment, text_style::TextAlign},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::integration::local::LocalFile;
use orchester_workspace::{
  display,
  tasks::local_integration::{IntegrationView, ShownSnapshot, Snapshot, SnapshotSize},
};

use super::{Selection, ShownFile};
use crate::{
  theme::{ExtraSpacing, LocalText},
  ui::{
    message::message_text,
    pressable::pressable,
    toggle::{Toggle, ToggleKind},
  },
};

/// Rows the list lays out in full; past them its rows scroll in a list `LIST_HEIGHT` high.
const SHOWN_ROWS: usize = 60;
const LIST_HEIGHT: f32 = 520.0;
/// The column of a change's letter, which centers it (`F1t7T` in `dAwTi`).
const KIND_WIDTH: f32 = 24.0;

/// Which list the page shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum List {
  /// Every snapshot, to choose what to commit or preserve.
  Review,
  /// The selected index and new-file snapshots a commit takes.
  Commit,
  /// The selected snapshots a preservation keeps.
  Preserve,
}

/// A group of rows; its key is its label and part of its rows' element ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Group {
  Staged,
  Unstaged,
  Untracked,
  CommitIndex,
  CommitUntracked,
}

impl Group {
  pub(super) const fn key(self) -> &'static str {
    match self {
      Self::Staged => "local_integration.staged",
      Self::Unstaged => "local_integration.unstaged",
      Self::Untracked => "local_integration.untracked",
      Self::CommitIndex => "local_integration.commitIndex",
      Self::CommitUntracked => "local_integration.commitUntracked",
    }
  }

  const fn snapshot(self) -> Snapshot {
    match self {
      Self::Staged | Self::CommitIndex => Snapshot::Staged,
      Self::Unstaged => Snapshot::Unstaged,
      Self::Untracked | Self::CommitUntracked => Snapshot::Untracked,
    }
  }
}

/// One listed snapshot.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Entry {
  pub group: Group,
  pub file: LocalFile,
  size: Option<SnapshotSize>,
}

fn untracked(file: &LocalFile) -> bool {
  file.head.is_none() && file.staged.is_none()
}

/// The snapshots `list` shows of `view`, in its order.
pub(super) fn entries(view: &IntegrationView, selection: &Selection, list: List) -> Vec<Entry> {
  let groups: &[Group] = match list {
    List::Review | List::Preserve => &[Group::Staged, Group::Unstaged, Group::Untracked],
    List::Commit => &[Group::CommitIndex, Group::CommitUntracked],
  };
  let kept = selection.paths();
  let mut entries = Vec::new();
  for &group in groups {
    for file in &view.checkout.files {
      let listed = match group {
        Group::Staged | Group::CommitIndex => file.staged != file.head,
        Group::Unstaged => file.unstaged,
        Group::Untracked | Group::CommitUntracked => untracked(file),
      };
      let chosen = match list {
        List::Review => true,
        List::Commit => selection.has(&file.path, group.snapshot()),
        // A preservation keeps a selected path whole: every snapshot of it is listed.
        List::Preserve => kept.contains(&file.path),
      };
      if listed && chosen {
        entries.push(Entry {
          group,
          file: file.clone(),
          size: view.stats.of(&file.path, group.snapshot()),
        });
      }
    }
  }
  entries
}

/// The snapshot the review shows first: the first working-copy one, as the person last edited it (`YxF6B`), or
/// else the first listed.
pub(super) fn first(view: &IntegrationView) -> Option<ShownSnapshot> {
  let listed = entries(view, &Selection::default(), List::Review);
  let first = listed
    .iter()
    .position(|entry| entry.group == Group::Unstaged)
    .unwrap_or(0);
  listed.into_iter().nth(first).map(|entry| ShownSnapshot {
    path: entry.file.path,
    snapshot: entry.group.snapshot(),
  })
}

/// Whether `shown` is still one of the review's snapshots.
pub(super) fn listed(view: &IntegrationView, shown: &ShownSnapshot) -> bool {
  entries(view, &Selection::default(), List::Review)
    .iter()
    .any(|entry| entry.file.path == shown.path && entry.group.snapshot() == shown.snapshot)
}

/// The review's snapshots that `selection` holds.
pub(super) fn selected_count(view: &IntegrationView, selection: &Selection) -> usize {
  entries(view, selection, List::Review)
    .iter()
    .filter(|entry| selection.has(&entry.file.path, entry.group.snapshot()))
    .count()
}

/// The snapshots a commit of `selection` leaves in the checkout, as the remaining notice names them.
pub(super) fn excluded(ctx: &Ctx, view: &IntegrationView, selection: &Selection) -> Vec<String> {
  let excluded = entries(view, &Selection::default(), List::Review)
    .into_iter()
    .filter(|entry| entry.group == Group::Unstaged || !selection.has(&entry.file.path, entry.group.snapshot()));
  named(ctx, excluded)
}

/// The snapshots a preservation of `selection` keeps though they were not selected: it keeps a selected path whole.
pub(super) fn kept_whole(ctx: &Ctx, view: &IntegrationView, selection: &Selection) -> Vec<String> {
  let kept = entries(view, selection, List::Preserve)
    .into_iter()
    .filter(|entry| !selection.has(&entry.file.path, entry.group.snapshot()));
  named(ctx, kept)
}

/// Each of `listed` by its group and path.
fn named(ctx: &Ctx, listed: impl Iterator<Item = Entry>) -> Vec<String> {
  listed
    .map(|entry| {
      ctx
        .t_args(
          "local_integration.excludedEntry",
          [
            ("group", &*ctx.t(entry.group.key())),
            ("path", entry.file.path.as_str()),
          ],
        )
        .to_string()
    })
    .collect()
}

/// The list `list` of `view` under `title`.
pub(super) fn list(
  ctx: &mut Ctx,
  (view, list): (&IntegrationView, List),
  (selected, shown): (&Signal<Selection>, &Signal<ShownFile>),
  title: &str,
  busy: bool,
) -> Element {
  let entries = entries(view, &selected.get(), list);
  let mut items = Vec::new();
  for entry in entries {
    if items.last().is_none_or(|item: &Item| item.group() != entry.group) {
      items.push(Item::Group(entry.group));
    }
    items.push(Item::File(entry));
  }
  let (selected, shown) = (selected.clone(), shown.clone());
  let props = move |item: Item| Props {
    item,
    list,
    selected: selected.clone(),
    shown: shown.clone(),
    busy,
  };
  let rows: Element = if items.len() <= SHOWN_ROWS {
    Column::new()
      .width(Dimension::Pct(100.0))
      .with_children(
        items
          .into_iter()
          .map(|item| ctx.mount_keyed::<FileRow>(&format!("local-row-{}", item.key()), props(item))),
      )
      .into()
  } else {
    VirtualizedList::new_shared(ctx, Arc::new(items))
      .list_key("local-files-rows")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(LIST_HEIGHT))
      .mount_keyed::<FileRow, _, _, _>(Item::key, move |item| props(item.clone()))
  };
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .child(Text::new(title).id("local-files-title").variant(TypographyStyle::Body));
  Column::new()
    .id("local-files")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .clip()
    .child(head)
    .child(rows)
    .into()
}

#[derive(Clone, Debug, PartialEq)]
enum Item {
  Group(Group),
  File(Entry),
}

impl Item {
  fn group(&self) -> Group {
    match self {
      Self::Group(group) => *group,
      Self::File(entry) => entry.group,
    }
  }

  fn key(&self) -> String {
    match self {
      Self::Group(group) => group.key().to_owned(),
      Self::File(entry) => format!("{}-{}", entry.group.key(), entry.file.path),
    }
  }
}

#[derive(Clone)]
struct Props {
  item: Item,
  list: List,
  selected: Signal<Selection>,
  shown: Signal<ShownFile>,
  busy: bool,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.item == other.item && self.list == other.list && self.busy == other.busy
  }
}

impl DevtoolsInspectable for Props {}

struct FileRow;

impl Component for FileRow {
  type Props = Props;

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    match &props.item {
      Item::Group(group) => Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(
          ExtraSpacing::Loose.role(),
          ExtraSpacing::Snug.role(),
        ))
        .background(PaletteColor::SurfaceRaised)
        .child(Text::new(&ctx.t(group.key())).variant(LocalText::Note.role()))
        .into(),
      Item::File(entry) => file_row(ctx, &props, entry),
    }
  }
}

fn file_row(ctx: &Ctx, props: &Props, entry: &Entry) -> Element {
  let group = entry.group.key();
  let path = entry.file.path.clone();
  let snapshot = entry.group.snapshot();
  // What a commit or preservation review lists, it takes.
  let on = props.list != List::Review || props.selected.get().has(&path, snapshot);
  let mut choice = Toggle::new(
    format!("local-select-{group}-{path}"),
    ToggleKind::Checkbox,
    on,
    path.to_string(),
  )
  .hide_label();
  if props.list != List::Review {
    choice = choice.settled();
  } else if !props.busy {
    let select = props.selected.clone();
    let path = path.clone();
    choice = choice.on_change(move |on| {
      select.update(|selection| selection.choose(&path, snapshot, on));
    });
  }
  let name = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(
      Text::new(kind(entry))
        .variant(LocalText::Letter.role())
        .text_align(TextAlign::Center)
        .width(Dimension::Px(KIND_WIDTH))
        .flex_shrink(0.0),
    )
    .child(
      Text::new(path.as_str())
        .id(format!("local-path-{group}-{path}"))
        .variant(LocalText::Path.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    );
  let name: Element = if props.list != List::Preserve {
    let shown = props.shown.clone();
    let snapshot = ShownSnapshot {
      path: path.clone(),
      snapshot: entry.group.snapshot(),
    };
    pressable(move || shown.set(ShownFile(Some(snapshot.clone()))))
      .id(format!("local-file-{group}-{path}"))
      .width(Dimension::Pct(100.0))
      .child(name)
      .into()
  } else {
    name.id(format!("local-file-{group}-{path}")).into()
  };
  Row::new()
    .id(format!("local-row-{group}-{path}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(choice)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(name)
        .child(
          Text::new(&caption(ctx, props.list, entry))
            .id(format!("local-caption-{group}-{path}"))
            .variant(LocalText::Note.role()),
        ),
    )
    .into()
}

/// The letter of how the snapshot changes its path: modified, added, deleted, or untracked.
fn kind(entry: &Entry) -> &'static str {
  let file = &entry.file;
  let (before, after) = match entry.group.snapshot() {
    Snapshot::Untracked => return "?",
    Snapshot::Staged => (file.head.is_some(), file.staged.is_some()),
    Snapshot::Unstaged => (file.staged.is_some(), file.working.is_some()),
  };
  match (before, after) {
    (false, _) => "A",
    (true, false) => "D",
    (true, true) => "M",
  }
}

fn caption(ctx: &Ctx, list: List, entry: &Entry) -> Arc<str> {
  let new_file = |text: &str, binary: &str, sized: &str| match entry.size {
    Some(SnapshotSize::Binary(Some(bytes))) => {
      let size = message_text(ctx, &display::byte_size(bytes));
      ctx.t_args(sized, [("size", &*size)])
    }
    Some(SnapshotSize::Binary(None)) => ctx.t(binary),
    _ => ctx.t(text),
  };
  match (list, entry.group) {
    (List::Commit, Group::CommitIndex) => ctx.t("local_integration.commitIndexCaption"),
    (List::Commit, _) => ctx.t("local_integration.commitUntrackedCaption"),
    (List::Preserve, Group::Staged) => ctx.t("local_integration.preserveStagedCaption"),
    (List::Preserve, Group::Unstaged) => ctx.t("local_integration.preserveUnstagedCaption"),
    (List::Preserve, _) => new_file(
      "local_integration.preserveNewCaption",
      "local_integration.preserveBinaryCaption",
      "local_integration.preserveBinarySizeCaption",
    ),
    (_, Group::Staged | Group::Unstaged) => {
      let (plain, sized) = if entry.group == Group::Staged {
        ("local_integration.stagedCaption", "local_integration.stagedSizeCaption")
      } else {
        (
          "local_integration.unstagedCaption",
          "local_integration.unstagedSizeCaption",
        )
      };
      match entry.size {
        Some(SnapshotSize::Lines { added, removed }) => {
          let change = ctx.t_args(
            "local_integration.lines",
            [
              ("added", added.to_string().as_str()),
              ("removed", removed.to_string().as_str()),
            ],
          );
          ctx.t_args(sized, [("change", &*change)])
        }
        Some(SnapshotSize::Binary(_)) => ctx.t_args(sized, [("change", &*ctx.t("local_integration.binaryChange"))]),
        _ => ctx.t(plain),
      }
    }
    (..) => new_file(
      "local_integration.newCaption",
      "local_integration.newBinaryCaption",
      "local_integration.newBinarySizeCaption",
    ),
  }
}
