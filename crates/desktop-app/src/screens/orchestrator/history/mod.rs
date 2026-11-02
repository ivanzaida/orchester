//! History (`History panel` `eFI39` in `04 History open` `l1ET7x`): the project's conversations in a 300-wide panel
//! over the chat's right edge, under the header, on `surface_panel` after a `border` line, its shadow cast to its
//! left. A search field at its top (12 in, 8 under it); the list, 8 in at its sides, its rows 2 apart: first the
//! conversations a person pinned, the latest pin first, under *Pinned* with an 11 pin 6 before it (`Pinned` `X981B` in
//! `08c` `r4DVU`), then the others by when each last changed (*Today*, *Yesterday*, *Earlier*: Geist Mono 10.5
//! `text_muted`, 10 over and 4 under), each a row ([`row`]) with its title (12.5/500, `text_primary` for the one shown,
//! else `text_secondary`) over when and how many messages (11 `text_muted`), the one shown on `hover`; and a note at
//! its foot over a `border` line. Opening one keeps the draft; Esc or the header's toggle closes it.

mod row;

use std::{cmp::Reverse, path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, VirtualizedList},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ChatId, ProjectId},
  time::Timestamp,
};
use orchester_store::ChatListing;

use self::row::ItemView;
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, OrchestratorShadow, OrchestratorText},
  ui::{
    field::FieldInput,
    icon::{Icon, icon},
    skeleton::{self, Skeleton, fading},
  },
};

/// The panel's width (`eFI39`) and a conversation's row's height (`aowsX`), and a day heading's bone (`ipw6U`).
const WIDTH: f32 = 300.0;
const ROW_HEIGHT: f32 = 46.0;
const DAY_BONE: f32 = 36.0;

/// When a conversation last changed, as the list groups it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Day {
  Today,
  Yesterday,
  Earlier,
}

impl Day {
  fn of(at: Timestamp, today: Timestamp) -> Self {
    let day = at.local_day_start();
    if day == today {
      Self::Today
    } else if day == Timestamp::from_unix_millis(today.unix_millis() - 1).local_day_start() {
      Self::Yesterday
    } else {
      Self::Earlier
    }
  }
}

/// The heading of a group of the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Heading {
  /// The conversations a person pinned.
  Pinned,
  /// The others, by when each last changed.
  Day(Day),
}

impl Heading {
  const fn key(self) -> &'static str {
    match self {
      Self::Pinned => "orchestrator.historyPanel.pinned",
      Self::Day(Day::Today) => "orchestrator.historyPanel.today",
      Self::Day(Day::Yesterday) => "orchestrator.historyPanel.yesterday",
      Self::Day(Day::Earlier) => "orchestrator.historyPanel.earlier",
    }
  }
}

/// The conversation whose title History edits in place; `None` while none is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Renaming(pub(super) Option<ChatId>);

impl lurq::app::component::DevtoolsInspectable for Renaming {}

/// What every conversation's row shares: the project, its folder, where *Rename* and *Pin* keep what they change, and
/// which conversation's title is edited in place, the screen's, which Esc cancels.
#[derive(Clone)]
pub(super) struct Rows {
  project: ProjectId,
  root: Arc<Path>,
  renaming: Signal<Renaming>,
}

impl Rows {
  pub(super) fn new(project: ProjectId, root: &Path, renaming: Signal<Renaming>) -> Self {
    Self {
      project,
      root: Arc::from(root),
      renaming,
    }
  }
}

/// Compared by what it names; the signal by which one it is.
impl PartialEq for Rows {
  fn eq(&self, other: &Self) -> bool {
    (self.project, &self.root, self.renaming.id()) == (other.project, &other.root, other.renaming.id())
  }
}

/// A row of the list: a group's heading or a conversation.
#[derive(Clone, PartialEq)]
enum Item {
  Heading(Heading),
  Chat {
    listing: ChatListing,
    current: bool,
    today: bool,
    rows: Rows,
  },
}

impl lurq::app::component::DevtoolsInspectable for Item {}

impl Item {
  fn key(&self) -> String {
    match self {
      Self::Heading(heading) => format!("heading-{heading:?}"),
      Self::Chat { listing, .. } => listing.chat.id().to_string(),
    }
  }
}

/// The panel over `chats` of the project `rows` name, `current` the one shown; `search` filters it by title. While
/// the conversations are read (`chats` none), the list is bones (`06 Orchestrator · History reading` `ipw6U`).
pub(super) fn panel(
  ctx: &mut Ctx,
  search: &Signal<String>,
  rows: &Rows,
  (chats, current): (Option<&[ChatListing]>, Option<ChatId>),
) -> Element {
  let label = ctx.t("orchestrator.historyPanel.search");
  let field = FieldInput::new("orchestrator-history-search", label.clone(), search.clone())
    .search()
    .placeholder(label)
    .render(ctx);
  let list: Element = match chats {
    None => reading(ctx),
    Some(chats) => list(ctx, chats, (current, rows), &search.get()),
  };
  Column::new()
    .id("orchestrator-history-panel")
    .width(Dimension::Px(WIDTH))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .box_shadow(OrchestratorShadow::History.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .top(SpacingSize::Md)
            .left(SpacingSize::Md)
            .right(SpacingSize::Md)
            .bottom(SpacingSize::Sm),
        )
        .child(field),
    )
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .padding(Padding::horizontal(SpacingSize::Sm))
        .child(list),
    )
    .child(foot(ctx))
    .into()
}

/// The conversations the search finds, the pinned ones and the others under their days, or why there are none.
fn list(ctx: &mut Ctx, chats: &[ChatListing], (current, rows): (Option<ChatId>, &Rows), search: &str) -> Element {
  let items = items(chats, (current, rows), search);
  if items.is_empty() {
    let words = if chats.is_empty() {
      "orchestrator.historyPanel.empty"
    } else {
      "orchestrator.historyPanel.noMatch"
    };
    return Column::new()
      .id("orchestrator-history-empty")
      .flex(1.0)
      .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Loose.role()))
      .child(Text::new(&ctx.t(words)).variant(TypographyStyle::Caption))
      .into();
  }
  VirtualizedList::new_shared(ctx, Arc::new(items))
    .list_key("orchestrator-history-list")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .mount_keyed::<ItemView, _, _, _>(Item::key, Clone::clone)
}

/// The list's lines while the conversations are read: a day's heading, or a conversation's title and meta.
const READING: [Option<(f32, f32)>; 11] = [
  None,
  Some((230.0, 100.0)),
  Some((164.0, 100.0)),
  None,
  Some((240.0, 100.0)),
  Some((186.0, 100.0)),
  None,
  Some((218.0, 100.0)),
  Some((250.0, 100.0)),
  Some((176.0, 100.0)),
  Some((150.0, 100.0)),
];

/// The list while it is read: the days' headings and the conversations' rows as bones in the rows' own geometry,
/// lower rows fading. *Reading conversations…* (`orchestrator.historyPanel.reading`) names it.
fn reading(ctx: &mut Ctx) -> Element {
  let lines = READING
    .iter()
    .map(|line| match line {
      None => Row::new()
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .left(SpacingSize::Sm)
            .right(SpacingSize::Sm)
            .top(ExtraSpacing::Loose.role())
            .bottom(SpacingSize::Xs),
        )
        .child(skeleton::line(ctx, OrchestratorText::Eyebrow.role(), DAY_BONE)),
      Some((title, meta)) => Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(ROW_HEIGHT))
        .padding(Padding::symmetric(
          ExtraSpacing::Loose.role(),
          ExtraSpacing::TextArea.role(),
        ))
        .align_items(Alignment::Center)
        .child(
          Column::new()
            .width(Dimension::Pct(100.0))
            .spacing(ExtraSpacing::Tight.role())
            .child(skeleton::line(ctx, OrchestratorText::Emphasis.role(), *title))
            .child(skeleton::line(ctx, OrchestratorText::HistoryMeta.role(), *meta)),
        ),
    })
    .collect();
  let bones = Column::new().width(Dimension::Pct(100.0)).with_children(fading(lines));
  Skeleton::new(
    "orchestrator-history-loading",
    ctx.t("orchestrator.historyPanel.reading"),
  )
  .fills()
  .show(ctx, bones)
}

/// The note at the panel's foot.
fn foot(ctx: &Ctx) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Loose.role()))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&ctx.t("orchestrator.historyPanel.note"))
        .id("orchestrator-history-note")
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
}

/// The list's rows: `chats` whose title has every word of `search`, the pinned ones first, the latest pin first, and
/// then the others under the day each last changed.
fn items(chats: &[ChatListing], (current, rows): (Option<ChatId>, &Rows), search: &str) -> Vec<Item> {
  let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
  let found = |listing: &&ChatListing| {
    let title = listing.chat.title().to_lowercase();
    words.iter().all(|word| title.contains(word.as_str()))
  };
  let today = Timestamp::now().local_day_start();
  let item = |listing: &ChatListing| Item::Chat {
    listing: listing.clone(),
    current: current == Some(listing.chat.id()),
    today: Day::of(listing.last, today) == Day::Today,
    rows: rows.clone(),
  };
  let mut pinned: Vec<&ChatListing> = chats
    .iter()
    .filter(found)
    .filter(|listing| listing.pinned.is_some())
    .collect();
  pinned.sort_by_key(|listing| Reverse(listing.pinned));
  let mut items = Vec::new();
  if !pinned.is_empty() {
    items.push(Item::Heading(Heading::Pinned));
    items.extend(pinned.into_iter().map(item));
  }
  let mut last_day = None;
  for listing in chats.iter().filter(found).filter(|listing| listing.pinned.is_none()) {
    let day = Day::of(listing.last, today);
    if last_day != Some(day) {
      items.push(Item::Heading(Heading::Day(day)));
      last_day = Some(day);
    }
    items.push(item(listing));
  }
  items
}

/// A group's heading: *Pinned* after its pin, or a day.
fn heading(ctx: &Ctx, heading: Heading) -> Element {
  Row::new()
    .id(format!("orchestrator-history-heading-{heading:?}"))
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm)
        .top(ExtraSpacing::Loose.role())
        .bottom(SpacingSize::Xs),
    )
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .with_children((heading == Heading::Pinned).then(|| icon(Icon::Pin, IconSize::Xs, PaletteColor::TextMuted)))
    .child(Text::new(&ctx.t(heading.key())).variant(OrchestratorText::Eyebrow.role()))
    .into()
}
