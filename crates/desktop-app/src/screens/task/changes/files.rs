//! The changed file list (`File list` `qT0kd`, 320 wide on the page with a `border` line at its end): the
//! run's checkpointed files under `CHECKPOINTED` and its count, then, under `NOT CHECKPOINTED`, what its
//! worktree holds beyond them, with where that was read and, while an agent works, how long ago (`T4jsZA`).
//! A file (`Changed file row` `dAwTi`) is its change's letter, its path cut where the row ends and its
//! added and removed counts; the selected one on `hover`. Virtualised, since a run may change hundreds of
//! files; its own component, so choosing a file re-renders only the list and the diff.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, VirtualizedList},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  diff::{FileChange, FileDiff},
  time::Timestamp,
};

use super::{listed::Listed, load::LastRead};
use crate::{
  routes::ChangeSource,
  theme::{CodeText, ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{pressable::pressable, tooltip::Tooltip},
};

/// Width of the file list (`qT0kd`), height of a file row (`dAwTi`) and width of its change's letter
/// (`F1t7T`).
pub const FILE_LIST_WIDTH: f32 = 320.0;
const FILE_HEIGHT: f32 = 30.0;
const LETTER_WIDTH: f32 = 12.0;
/// What a file row leaves its path of the list's width besides the counts: the list's padding (2 × 8),
/// the row's (2 × 10), the letter (12) and the gaps after the letter and the path (2 × 8).
const ROW_BESIDES_PATH: f32 = 64.0;
/// A count's gap before it (8), and the advances of Geist Mono at 12 (a path) and 11 (a count): 0.6 em.
const COUNT_GAP: f32 = 8.0;
const PATH_CHAR_WIDTH: f32 = 7.2;
const COUNT_CHAR_WIDTH: f32 = 6.6;
/// The fewest characters a cut path keeps of its start.
const CUT_HEAD: usize = 4;
/// How often the line saying when the worktree was read is brought up to date.
const AGE_TICK: Duration = Duration::from_secs(1);

/// The letter of a file's change and its color: an added file's in `success`, a deleted one's in
/// `danger`, any other in `text_secondary` (`F1t7T`).
pub fn change_letter(ctx: &Ctx, change: &FileChange) -> (Arc<str>, PaletteColor) {
  let (key, color) = match change {
    FileChange::Added => ("changes.type.added", PaletteColor::Success),
    FileChange::Deleted => ("changes.type.deleted", PaletteColor::Danger),
    FileChange::Modified => ("changes.type.modified", PaletteColor::TextSecondary),
    FileChange::Renamed { .. } => ("changes.type.renamed", PaletteColor::TextSecondary),
    FileChange::Copied { .. } => ("changes.type.copied", PaletteColor::TextSecondary),
  };
  (ctx.t(key), color)
}

/// What the list and the diff's head show of a file: its change and how many lines it adds and removes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSummary {
  pub change: FileChange,
  pub additions: u32,
  pub deletions: u32,
  pub binary: bool,
}

impl FileSummary {
  pub fn of(file: &FileDiff) -> Self {
    Self {
      change: file.change().clone(),
      additions: file.additions(),
      deletions: file.deletions(),
      binary: file.is_binary(),
    }
  }

  /// The added lines in `success` and the removed ones in `danger`, each only when there are some
  /// (`dXDtc`, `zcx5G`); a binary file has neither.
  pub fn counts(&self, ctx: &Ctx, typography: TypographyStyle) -> Vec<Element> {
    self
      .count_texts(ctx)
      .into_iter()
      .zip([PaletteColor::Success, PaletteColor::Danger].into_iter().cycle())
      .map(|(text, color)| Text::new(&text).variant(typography).color(color).nowrap().into())
      .collect()
  }

  fn count_texts(&self, ctx: &Ctx) -> Vec<Arc<str>> {
    if self.binary {
      return Vec::new();
    }
    [(self.additions, "changes.added"), (self.deletions, "changes.removed")]
      .into_iter()
      .filter(|(count, _)| *count > 0)
      .map(|(count, key)| ctx.t_args(key, [("count", count.to_string())]))
      .collect()
  }
}

/// What the line under `NOT CHECKPOINTED` says.
#[derive(Clone, PartialEq)]
pub struct WorktreeNote {
  /// The commit the worktree's changes start from, short.
  pub revision: Arc<str>,
  /// While an agent works: its name, and when the worktree was last read.
  pub live: Option<(Option<Arc<str>>, LastRead)>,
}

/// A group's heading, one of its files, or the room between two groups.
#[derive(Clone, PartialEq)]
pub enum FileItem {
  Group {
    source: ChangeSource,
    count: usize,
    note: Option<WorktreeNote>,
  },
  File {
    /// Position in the list.
    position: usize,
    listed: Listed,
    summary: FileSummary,
    selected: bool,
  },
  Gap,
}

impl FileItem {
  fn key(&self) -> String {
    match self {
      Self::Group { source, .. } => format!("group:{}", source.name()),
      Self::File { position, .. } => format!("file:{position}"),
      Self::Gap => "gap".to_owned(),
    }
  }
}

/// Called with the file a person chose.
pub type SelectFile = Arc<dyn Fn(&Listed) + Send + Sync>;

#[derive(Clone)]
pub struct FileListProps {
  pub items: Arc<Vec<FileItem>>,
  pub select: SelectFile,
}

impl PartialEq for FileListProps {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.items, &other.items)
  }
}

impl lurq::app::component::DevtoolsInspectable for FileListProps {}

pub struct FileList;

impl Component for FileList {
  type Props = FileListProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let FileListProps { items, select } = ctx.props::<FileListProps>().clone();
    let list: Element = VirtualizedList::new_shared(ctx, items)
      .list_key("changes-files")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .mount_keyed::<FileRow, _, _, _>(FileItem::key, move |item| FileRowProps {
        item: item.clone(),
        select: select.clone(),
      });
    Column::new()
      .id("changes-files")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Xs))
      .child(list)
  }
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
    match item {
      FileItem::Group { source, count, note } => group(ctx, source, count, note.as_ref()),
      FileItem::File {
        listed,
        summary,
        selected,
        ..
      } => file_row(ctx, &listed, &summary, selected, select),
      // The room between two groups (`m7oDak`).
      FileItem::Gap => Column::new().padding_top(SpacingSize::Sm).into(),
    }
  }
}

/// A group's heading and count, and under `NOT CHECKPOINTED` where its changes were read (`hyGiS`).
fn group(ctx: &mut Ctx, source: ChangeSource, count: usize, note: Option<&WorktreeNote>) -> Element {
  let (id, key) = match source {
    ChangeSource::Checkpoint => ("changes-group-checkpointed", "changes.files.checkpointed"),
    ChangeSource::Worktree => ("changes-group-worktree", "changes.files.notCheckpointed"),
  };
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t(key)).variant(CodeText::GroupHeading.role()).nowrap())
    .child(
      Text::new(&count.to_string())
        .variant(CodeText::GroupCount.role())
        .nowrap(),
    );
  let note = note.map(|note| match &note.live {
    Some((agent, last_read)) => ctx.mount_keyed::<ReadAge>(
      "changes-worktree-read",
      ReadAgeProps {
        revision: note.revision.clone(),
        agent: agent.clone(),
        last_read: last_read.clone(),
      },
    ),
    None => note_text(&ctx.t_args("changes.files.worktreeAfter", [("revision", &*note.revision)])),
  });
  Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Xs).top(SpacingSize::Md))
    .spacing(ExtraSpacing::Tight.role())
    .child(head)
    .with_children(note)
    .into()
}

fn note_text(text: &str) -> Element {
  Text::new(text)
    .id("changes-worktree-note")
    .variant(TypographyStyle::Caption)
    .width(Dimension::Pct(100.0))
    .into()
}

/// A changed file (`dAwTi`); the selected one on `hover`, the others take it under the pointer.
fn file_row(ctx: &mut Ctx, listed: &Listed, summary: &FileSummary, selected: bool, select: SelectFile) -> Element {
  let prefix = match listed.source {
    ChangeSource::Checkpoint => "changes-file",
    ChangeSource::Worktree => "changes-worktree-file",
  };
  let id = format!("{prefix}:{}", listed.path);
  let (letter, color) = change_letter(ctx, &summary.change);
  let path = path_text(ctx, &format!("{id}-path"), &listed.path, summary);
  let content = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&letter)
        .variant(CodeText::ChangeType.role())
        .color(color)
        .nowrap()
        .width(Dimension::Px(LETTER_WIDTH)),
    )
    .child(Column::new().flex(1.0).child(path))
    .with_children(summary.counts(ctx, ExtraTypography::MonoNote.role()));
  let path = listed.path.to_string();
  let listed = listed.clone();
  let row = pressable(move || select(&listed))
    .id(id)
    .describe("path", path)
    .describe("state", if selected { "selected" } else { "enabled" })
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FILE_HEIGHT))
    .rounded(RadiusSize::Md)
    .child(content);
  if selected {
    row.background(ExtraColor::Hover.role()).into()
  } else {
    row
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .into()
  }
}

/// The path in the room its row leaves it, cut in the middle so its file name stays
/// (`migrations/0042_…_attempts.sql` in `qT0kd`); while it is cut, its whole path shows in a tooltip.
fn path_text(ctx: &mut Ctx, id: &str, path: &str, summary: &FileSummary) -> Element {
  let counts: f32 = summary
    .count_texts(ctx)
    .iter()
    .map(|count| COUNT_GAP + chars(count) * COUNT_CHAR_WIDTH)
    .sum();
  let room = (FILE_LIST_WIDTH - ROW_BESIDES_PATH - counts) / PATH_CHAR_WIDTH;
  #[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a row's room is a few dozen characters, never negative after `max`"
  )]
  let room = room.floor().max(0.0) as usize;
  let shown = middle_cut(path, room);
  let text = Text::new(&shown)
    .id(id.to_owned())
    .variant(TypographyStyle::Mono)
    .nowrap();
  if shown == path {
    return text.into();
  }
  Tooltip::new(format!("{id}-tooltip"), path.to_owned()).wrap(ctx, text)
}

#[allow(
  clippy::cast_precision_loss,
  reason = "a count holds a few characters, far inside f32's exact range"
)]
fn chars(text: &str) -> f32 {
  text.chars().count() as f32
}

/// `path` in at most `room` characters: whole when it fits, otherwise its start, `…` and its end, the
/// end long enough for its file name when there is room for that.
pub fn middle_cut(path: &str, room: usize) -> String {
  let characters: Vec<char> = path.chars().collect();
  if characters.len() <= room || room <= CUT_HEAD + 1 {
    return path.to_owned();
  }
  let name = path.rsplit('/').next().unwrap_or(path).chars().count();
  let tail = name.max((room - 1) / 2).min(room - 1 - CUT_HEAD);
  let head = room - 1 - tail;
  let mut cut: String = characters[..head].iter().collect();
  cut.push('…');
  cut.extend(&characters[characters.len() - tail..]);
  cut
}

#[derive(Clone, PartialEq)]
struct ReadAgeProps {
  revision: Arc<str>,
  agent: Option<Arc<str>>,
  last_read: LastRead,
}

impl lurq::app::component::DevtoolsInspectable for ReadAgeProps {}

/// Where the worktree's changes start, and how long ago it was read while an agent works in it (`DVwZI`):
/// a component of its own with its own interval, so only this line renders each second.
struct ReadAge {
  /// The time it was brought up to date, in Unix milliseconds.
  now: Signal<i64>,
}

impl Component for ReadAge {
  type Props = ReadAgeProps;

  fn create(ctx: &mut Ctx) -> Self {
    let now = ctx.signal(Timestamp::now().unix_millis());
    let tick = now.clone();
    ctx
      .create_interval(AGE_TICK, move || tick.set(Timestamp::now().unix_millis()))
      .start();
    Self { now }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ReadAgeProps>().clone();
    let now = self.now.get();
    let Some(read) = props.last_read.get() else {
      return note_text(&ctx.t_args("changes.files.worktreeAfter", [("revision", &*props.revision)]));
    };
    let seconds = (now - read.unix_millis()).max(0) / 1000;
    let seconds = seconds.to_string();
    let text = match &props.agent {
      Some(agent) => ctx.t_args(
        "changes.files.worktreeReadWhileAgent",
        [("revision", &*props.revision), ("seconds", &seconds), ("agent", agent)],
      ),
      None => ctx.t_args(
        "changes.files.worktreeRead",
        [("revision", &*props.revision), ("seconds", &seconds)],
      ),
    };
    note_text(&text)
  }
}
