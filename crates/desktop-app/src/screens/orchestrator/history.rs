//! History (`History panel` `eFI39` in `04 History open` `l1ET7x`): the project's conversations in a 300-wide panel
//! over the chat's right edge, under the header, on `surface_panel` after a `border` line, its shadow cast to its
//! left. A search field at its top (12 in, 8 under it); the list, 8 in at its sides, its rows 2 apart, by when each
//! conversation last changed (*Today*, *Yesterday*, *Earlier*: Geist Mono 10.5 `text_muted`, 10 over and 4 under),
//! each a row (radius 6, 7 by 10 inside) with its title (12.5/500, `text_primary` for the one shown, else
//! `text_secondary`) over when and how many messages (11 `text_muted`), the one shown on `hover`; and a note at its
//! foot over a `border` line. Opening one keeps the draft; Esc or the header's toggle closes it.

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
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ChatId, ProjectId},
  time::Timestamp,
};
use orchester_store::ChatListing;
use orchester_workspace::display;

use crate::{
  navigation::Navigation,
  routes::{OrchestratorChat, ProjectPage, Route},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, OrchestratorShadow, OrchestratorText},
  ui::{
    field::FieldInput,
    pressable::opening,
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

  const fn key(self) -> &'static str {
    match self {
      Self::Today => "orchestrator.historyPanel.today",
      Self::Yesterday => "orchestrator.historyPanel.yesterday",
      Self::Earlier => "orchestrator.historyPanel.earlier",
    }
  }
}

/// A row of the list: a day's heading or a conversation.
#[derive(Clone, PartialEq)]
enum Item {
  Day(Day),
  Chat {
    listing: ChatListing,
    current: bool,
    today: bool,
    project: ProjectId,
  },
}

impl lurq::app::component::DevtoolsInspectable for Item {}

impl Item {
  fn key(&self) -> String {
    match self {
      Self::Day(day) => format!("day-{day:?}"),
      Self::Chat { listing, .. } => listing.chat.id().to_string(),
    }
  }
}

/// The panel over `chats` of the project, `current` the one shown; `search` filters it by title. While the
/// conversations are read (`chats` none), the list is bones (`06 Orchestrator · History reading` `ipw6U`).
pub(super) fn panel(
  ctx: &mut Ctx,
  search: &Signal<String>,
  project: ProjectId,
  (chats, current): (Option<&[ChatListing]>, Option<ChatId>),
) -> Element {
  let label = ctx.t("orchestrator.historyPanel.search");
  let field = FieldInput::new("orchestrator-history-search", label.clone(), search.clone())
    .search()
    .placeholder(label)
    .render(ctx);
  let list: Element = match chats {
    None => reading(ctx),
    Some(chats) => list(ctx, chats, (current, project), &search.get()),
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

/// The conversations the search finds, under their days, or why there are none.
fn list(
  ctx: &mut Ctx,
  chats: &[ChatListing],
  (current, project): (Option<ChatId>, ProjectId),
  search: &str,
) -> Element {
  let items = items(chats, (current, project), search);
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

/// The list's rows: `chats` whose title has every word of `search`, under the day each last changed.
fn items(chats: &[ChatListing], (current, project): (Option<ChatId>, ProjectId), search: &str) -> Vec<Item> {
  let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
  let today = Timestamp::now().local_day_start();
  let mut items = Vec::new();
  let mut last_day = None;
  for listing in chats {
    let title = listing.chat.title().to_lowercase();
    if !words.iter().all(|word| title.contains(word.as_str())) {
      continue;
    }
    let day = Day::of(listing.last, today);
    if last_day != Some(day) {
      items.push(Item::Day(day));
      last_day = Some(day);
    }
    items.push(Item::Chat {
      listing: listing.clone(),
      current: current == Some(listing.chat.id()),
      today: day == Day::Today,
      project,
    });
  }
  items
}

struct ItemView;

impl Component for ItemView {
  type Props = Item;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    match ctx.props::<Item>().clone() {
      Item::Day(day) => Row::new()
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .left(SpacingSize::Sm)
            .right(SpacingSize::Sm)
            .top(ExtraSpacing::Loose.role())
            .bottom(SpacingSize::Xs),
        )
        .child(Text::new(&ctx.t(day.key())).variant(OrchestratorText::Eyebrow.role()))
        .into(),
      Item::Chat {
        listing,
        current,
        today,
        project,
      } => chat_row(ctx, &listing, (current, today), project),
    }
  }
}

/// A conversation's row; pressing it opens the conversation.
fn chat_row(ctx: &mut Ctx, listing: &ChatListing, (current, today): (bool, bool), project: ProjectId) -> Element {
  let id = listing.chat.id();
  let when: Arc<str> = if today {
    display::clock(listing.last).into()
  } else {
    super::date(ctx, listing.last)
  };
  let count = listing.messages.to_string();
  let messages = if listing.messages == 1 {
    ctx.t("orchestrator.historyPanel.messages.one")
  } else {
    ctx.t_args("orchestrator.historyPanel.messages.other", [("count", count.as_str())])
  };
  let meta = ctx.t_args(
    "orchestrator.historyPanel.meta",
    [("when", &*when), ("messages", &*messages)],
  );
  let navigation = Navigation::of(ctx);
  let route = Route::Project(project, ProjectPage::Orchestrator(OrchestratorChat::Chat(id)));
  let row = opening(navigation.to(route))
    .id(format!("orchestrator-history-{id}"))
    .describe("current", current.to_string())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding(Padding::symmetric(
      ExtraSpacing::Loose.role(),
      ExtraSpacing::TextArea.role(),
    ))
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()));
  let row = if current {
    row.background(ExtraColor::Hover.role())
  } else {
    row
  };
  let title_color = if current {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  row
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(listing.chat.title())
            .variant(OrchestratorText::Emphasis.role())
            .color(title_color)
            .width(Dimension::Pct(100.0))
            .nowrap()
            .text_overflow(TextOverflow::Elipsis),
        )
        .child(
          Text::new(&meta)
            .variant(OrchestratorText::HistoryMeta.role())
            .nowrap()
            .text_overflow(TextOverflow::Elipsis),
        ),
    )
    .into()
}
