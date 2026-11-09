//! History (`History panel` `eFI39` in `04 History open` `l1ET7x`): the project's conversations in a 300-wide panel
//! over the chat's right edge, under the header, on `surface_panel` after a `border` line, its shadow cast to its
//! left. A search field at its top (12 in, 8 under it); the list, 8 in at its sides, its rows 2 apart: first the
//! conversations a person pinned, the latest pin first, under *Pinned* with an 11 pin 6 before it (`Pinned` `X981B` in
//! `08c` `r4DVU`), then the others by when each last changed (*Today*, *Yesterday*, *Earlier*: Geist Mono 10.5
//! `text_muted`, 10 over and 4 under), each a row ([`row`]) with its title (12.5/500, `text_primary` for the one shown,
//! else `text_secondary`) over when and how many messages (11 `text_muted`), or after an eye what it leads and when
//! (`i687ei` c, [`meta`]), the one shown on `hover`; and at its foot
//! over a `border` line, the way to the archived conversations while there are some (`Archived entry` `Z6bee` in `08`
//! `q30JYF`: an archive glyph, *Archived* and their count, a chevron at its end) over a note. The archived ones are a
//! view of their own ([`archived`], `08d` `qAfeu`); deleting one asks in a modal first ([`delete`], `08e` `I4xzOf`).
//! The search finds the words in the titles and in the messages ([`search`]); a result found in its messages opens
//! at the first of them. Opening one keeps the draft; Esc or the header's toggle closes it.

mod archived;
mod delete;
mod meta;
mod row;
mod search;

use std::{cmp::Reverse, path::Path, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, VirtualizedList},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ChatId, ProjectId},
  time::Timestamp,
};
use orchester_store::ChatListing;
use orchester_workspace::{
  orchestrator::{chat::History, watching::LeadCounts},
  tasks::timeline::find::Needle,
};

use self::{
  row::ItemView,
  search::{Matched, Outcome, RunProps, Search, SearchRun, Searched},
};
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, OrchestratorShadow, OrchestratorText},
  ui::{
    field::FieldInput,
    icon::{Icon, icon},
    pressable::pressable,
    skeleton::{self, Skeleton, fading},
  },
};

/// The panel's width (`eFI39`) and a conversation's row's height (`aowsX`), a result's with its matching line (`qFCmX`
/// in 13 `SZ6QY`), and a day heading's bone (`ipw6U`).
const WIDTH: f32 = 300.0;
const ROW_HEIGHT: f32 = 46.0;
const FOUND_ROW_HEIGHT: f32 = 66.0;
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

/// The conversation whose deletion the modal asks about; `None` while it asks about none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Deleting(pub(super) Option<ChatId>);

impl lurq::app::component::DevtoolsInspectable for Deleting {}

/// A conversation opened from a result of History's search found in its messages: it opens at entry `seq`, the one
/// its first matching line is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OpenedAt {
  pub(super) chat: ChatId,
  pub(super) seq: i64,
}

impl lurq::app::component::DevtoolsInspectable for OpenedAt {}

/// What History keeps while the screen shows it, the screen's, as Esc goes back through it: the delete asked about,
/// the title edited in place, whether it shows the archived conversations, the words each list is searched by, what
/// the search found in the messages so far, and the conversation last opened at a match.
#[derive(Clone)]
pub(super) struct HistoryState {
  pub(super) deleting: Signal<Deleting>,
  pub(super) renaming: Signal<Renaming>,
  pub(super) archived: Signal<bool>,
  pub(super) search: Signal<String>,
  pub(super) archived_search: Signal<String>,
  outcome: Signal<Outcome>,
  pub(super) opened_at: Signal<Option<OpenedAt>>,
}

impl HistoryState {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      deleting: ctx.signal(Deleting::default()),
      renaming: ctx.signal(Renaming::default()),
      archived: ctx.signal(false),
      search: ctx.signal(String::new()),
      archived_search: ctx.signal(String::new()),
      outcome: ctx.signal(Outcome::default()),
      opened_at: ctx.signal(None),
    }
  }
}

/// What every conversation's row shares: the project, its folder, where *Rename*, *Pin*, *Archive* and *Delete* keep
/// what they change and where a result opened at a match says so, and History's state, the screen's.
#[derive(Clone)]
pub(super) struct Rows {
  project: ProjectId,
  root: Arc<Path>,
  renaming: Signal<Renaming>,
  deleting: Signal<Deleting>,
  archived: Signal<bool>,
  opened_at: Signal<Option<OpenedAt>>,
}

impl Rows {
  pub(super) fn new(project: ProjectId, root: &Path, state: &HistoryState) -> Self {
    Self {
      project,
      root: Arc::from(root),
      renaming: state.renaming.clone(),
      deleting: state.deleting.clone(),
      archived: state.archived.clone(),
      opened_at: state.opened_at.clone(),
    }
  }
}

/// Compared by what it names; the signals by which ones they are.
impl PartialEq for Rows {
  fn eq(&self, other: &Self) -> bool {
    let signals = |rows: &Self| {
      (
        rows.renaming.id(),
        rows.deleting.id(),
        rows.archived.id(),
        rows.opened_at.id(),
      )
    };
    (self.project, &self.root, signals(self)) == (other.project, &other.root, signals(other))
  }
}

/// A row of the list: a group's heading or a conversation, with what the search found in it while it searches and
/// what it leads.
#[derive(Clone, PartialEq)]
enum Item {
  Heading(Heading),
  Chat {
    listing: Box<ChatListing>,
    current: bool,
    today: bool,
    matched: Option<Matched>,
    leads: LeadCounts,
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

/// The panel over the conversations of the project `rows` name, `current` the one shown: History's list, or the
/// archived ones while `state` says so; each list is searched by its own words. While the conversations are read
/// (`history` none), the list is bones (`06 Orchestrator · History reading` `ipw6U`). The delete it asks about opens
/// over the window.
pub(super) fn panel(
  ctx: &mut Ctx,
  state: &HistoryState,
  rows: &Rows,
  (history, current): (Option<&History>, Option<ChatId>),
) -> Element {
  let asked = state
    .deleting
    .get()
    .0
    .and_then(|id| history?.chats.iter().find(|listing| listing.chat.id() == id));
  let dialog = asked.map(|listing| delete::dialog(ctx, listing, rows, current == Some(listing.chat.id())));
  let archived = history.map(History::archived).unwrap_or_default();
  let (head, list, foot) = if state.archived.get() {
    archived::parts(ctx, state, rows, (&archived, current))
  } else {
    main_parts(ctx, state, rows, (history, current), archived.len())
  };
  Column::new()
    .id("orchestrator-history-panel")
    .width(Dimension::Px(WIDTH))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .box_shadow(OrchestratorShadow::History.role())
    .child(head)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .padding(Padding::horizontal(SpacingSize::Sm))
        .child(list),
    )
    .child(foot)
    .with_children(dialog)
    .into()
}

/// History's list: its search field, and under it while it searches what it found, the conversations `history` lists
/// (none while they are read) and its foot, the way to the `archived` ones while there are some over the note. The
/// messages of the conversations it lists are searched as the words in the field change.
fn main_parts(
  ctx: &mut Ctx,
  state: &HistoryState,
  rows: &Rows,
  (history, current): (Option<&History>, Option<ChatId>),
  archived: usize,
) -> (Element, Element, Element) {
  let label = ctx.t("orchestrator.historyPanel.search");
  let field = FieldInput::new("orchestrator-history-search", label.clone(), state.search.clone())
    .search()
    .placeholder(label)
    .render(ctx);
  let needle = Needle::new(&state.search.get());
  let chats: Option<Vec<&ChatListing>> = history.map(|history| history.listed().collect());
  let searched: Option<Searched> = chats
    .as_ref()
    .map(|chats| chats.iter().map(|listing| (listing.chat.id(), listing.last)).collect());
  let run = searched.clone().map(|chats| {
    let props = RunProps {
      root: rows.root.clone(),
      chats,
      needle: needle.clone(),
      outcome: state.outcome.clone(),
    };
    ctx.mount_keyed::<SearchRun>("orchestrator-history-search-run", props)
  });
  let search = needle
    .zip(searched)
    .map(|(needle, chats)| Search::of(needle, &chats, &state.outcome.get()));
  let (list, status) = match history.zip(chats.as_deref()) {
    None => (reading(ctx), None),
    Some((history, chats)) => {
      let items = items((chats, history), (current, rows), search.as_ref());
      let results = items.iter().filter(|item| matches!(item, Item::Chat { .. })).count();
      let status = search.as_ref().map(|search| search.status(ctx, results));
      let words = if chats.is_empty() {
        "orchestrator.historyPanel.empty"
      } else {
        "orchestrator.historyPanel.noMatch"
      };
      (list(ctx, items, ("orchestrator-history-list", words)), status)
    }
  };
  let bottom = if status.is_some() {
    SpacingSize::Xs
  } else {
    SpacingSize::Sm
  };
  let head = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Md)
        .left(SpacingSize::Md)
        .right(SpacingSize::Md)
        .bottom(bottom),
    )
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .child(field)
        .with_children(run),
    )
    .with_children(status);
  (head.into(), list, foot(ctx, rows, archived).into())
}

/// `items`, or the words `empty` says why there are none with.
fn list(ctx: &mut Ctx, items: Vec<Item>, (key, empty): (&str, &str)) -> Element {
  if items.is_empty() {
    return Column::new()
      .id("orchestrator-history-empty")
      .flex(1.0)
      .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Loose.role()))
      .child(Text::new(&ctx.t(empty)).variant(TypographyStyle::Caption))
      .into();
  }
  VirtualizedList::new_shared(ctx, Arc::new(items))
    .list_key(key)
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

/// The panel's foot: the way to the `archived` conversations while there are some, over the note.
fn foot(ctx: &Ctx, rows: &Rows, archived: usize) -> Column {
  let foot = Column::new()
    .id("orchestrator-history-foot")
    .width(Dimension::Pct(100.0))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border));
  let note = Text::new(&ctx.t("orchestrator.historyPanel.note"))
    .id("orchestrator-history-note")
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0));
  if archived == 0 {
    return foot
      .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Loose.role()))
      .child(note);
  }
  let open = rows.archived.clone();
  let entry = pressable(move || open.set(true))
    .id("orchestrator-history-archived")
    .describe("a11y_name", ctx.t("orchestrator.historyPanel.archived"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(Icon::Archive, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("orchestrator.historyPanel.archived"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(Text::new(&archived.to_string()).variant(ExtraTypography::MonoNote.role()))
    .child(Row::new().flex(1.0))
    .child(icon(Icon::ChevronRight, IconSize::Base, PaletteColor::TextMuted));
  foot
    .padding(
      Padding::new()
        .top(ExtraSpacing::Snug.role())
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm)
        .bottom(ExtraSpacing::Loose.role()),
    )
    .spacing(ExtraSpacing::Snug.role())
    .child(entry)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::horizontal(ExtraSpacing::TextArea.role()))
        .child(note),
    )
}

/// Whether the title of `listing` has every word of `words`, which are lowercase.
fn found(listing: &ChatListing, words: &[String]) -> bool {
  let title = listing.chat.title().to_lowercase();
  words.iter().all(|word| title.contains(word.as_str()))
}

/// The words of `search`, lowercase, as [`found`] takes them.
fn search_words(search: &str) -> Vec<String> {
  search.split_whitespace().map(str::to_lowercase).collect()
}

/// The row of `listing` of the project `rows` name, `current` the one shown; an archived one says when it was archived
/// rather than what it leads.
fn chat_item(listing: &ChatListing, (current, rows): (Option<ChatId>, &Rows), today: Timestamp) -> Item {
  found_item(listing, (current, rows), today, (None, LeadCounts::default()))
}

/// [`chat_item`] with what the search `matched` in it and what it `leads`.
fn found_item(
  listing: &ChatListing,
  (current, rows): (Option<ChatId>, &Rows),
  today: Timestamp,
  (matched, leads): (Option<Matched>, LeadCounts),
) -> Item {
  Item::Chat {
    listing: Box::new(listing.clone()),
    current: current == Some(listing.chat.id()),
    today: Day::of(listing.last, today) == Day::Today,
    matched,
    leads,
    rows: rows.clone(),
  }
}

/// The list's rows: `chats` of `history`, or while it searches those `search` found in their title or their messages,
/// the pinned ones first, the latest pin first, and then the others under the day each last changed; each with what it
/// leads.
fn items(
  (chats, history): (&[&ChatListing], &History),
  shown: (Option<ChatId>, &Rows),
  search: Option<&Search>,
) -> Vec<Item> {
  let chats: Vec<(&ChatListing, Option<Matched>)> = chats
    .iter()
    .filter_map(|&listing| match search {
      None => Some((listing, None)),
      Some(search) => search
        .matched(listing.chat.id(), listing.chat.title())
        .map(|matched| (listing, Some(matched))),
    })
    .collect();
  let today = Timestamp::now().local_day_start();
  let item = |(listing, matched): (&ChatListing, Option<Matched>)| {
    let leads = history.leads_of(listing.chat.id());
    found_item(listing, shown, today, (matched, leads))
  };
  let (mut pinned, others): (Vec<_>, Vec<_>) = chats.into_iter().partition(|(listing, _)| listing.pinned.is_some());
  pinned.sort_by_key(|(listing, _)| Reverse(listing.pinned));
  let mut items = Vec::new();
  if !pinned.is_empty() {
    items.push(Item::Heading(Heading::Pinned));
    items.extend(pinned.into_iter().map(item));
  }
  let mut last_day = None;
  for chat in others {
    let day = Day::of(chat.0.last, today);
    if last_day != Some(day) {
      items.push(Item::Heading(Heading::Day(day)));
      last_day = Some(day);
    }
    items.push(item(chat));
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
